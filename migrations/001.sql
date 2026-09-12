CREATE TABLE IF NOT EXISTS traces (
 id TEXT PRIMARY KEY, workflow_name TEXT NOT NULL, group_id TEXT, metadata TEXT NOT NULL DEFAULT '{}', raw TEXT NOT NULL,
 first_seen INTEGER NOT NULL, last_seen INTEGER NOT NULL,
 span_count INTEGER NOT NULL DEFAULT 0, error_count INTEGER NOT NULL DEFAULT 0, unfinished_count INTEGER NOT NULL DEFAULT 0,
 generation_count INTEGER NOT NULL DEFAULT 0, function_count INTEGER NOT NULL DEFAULT 0,
 started_at INTEGER, ended_at INTEGER
);
CREATE INDEX IF NOT EXISTS traces_recent ON traces(last_seen DESC,id DESC);
CREATE INDEX IF NOT EXISTS traces_errors ON traces(last_seen DESC,id DESC) WHERE error_count>0;
CREATE INDEX IF NOT EXISTS traces_workflow ON traces(workflow_name,last_seen DESC);
CREATE INDEX IF NOT EXISTS traces_group ON traces(group_id,last_seen DESC);
CREATE TABLE IF NOT EXISTS spans (
 id TEXT PRIMARY KEY, trace_id TEXT NOT NULL REFERENCES traces(id) ON DELETE CASCADE, parent_id TEXT,
 span_type TEXT NOT NULL, name TEXT, model TEXT, started_at INTEGER, ended_at INTEGER, duration_ms INTEGER,
 has_error INTEGER NOT NULL, error_text TEXT NOT NULL, input_tokens INTEGER NOT NULL DEFAULT 0, output_tokens INTEGER NOT NULL DEFAULT 0,
 raw TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS spans_trace_time ON spans(trace_id,started_at,id);
CREATE INDEX IF NOT EXISTS spans_trace_type ON spans(trace_id,span_type);
CREATE INDEX IF NOT EXISTS spans_trace_model ON spans(trace_id,model);
CREATE TRIGGER IF NOT EXISTS spans_insert AFTER INSERT ON spans BEGIN
 UPDATE traces SET span_count=span_count+1,error_count=error_count+NEW.has_error,
 unfinished_count=unfinished_count+(NEW.ended_at IS NULL),generation_count=generation_count+(NEW.span_type='generation'),
 function_count=function_count+(NEW.span_type='function') WHERE id=NEW.trace_id;
END;
CREATE TRIGGER IF NOT EXISTS spans_delete AFTER DELETE ON spans BEGIN
 UPDATE traces SET span_count=span_count-1,error_count=error_count-OLD.has_error,
 unfinished_count=unfinished_count-(OLD.ended_at IS NULL),generation_count=generation_count-(OLD.span_type='generation'),
 function_count=function_count-(OLD.span_type='function') WHERE id=OLD.trace_id;
END;
CREATE TRIGGER IF NOT EXISTS spans_update AFTER UPDATE ON spans BEGIN
 UPDATE traces SET span_count=span_count-1,error_count=error_count-OLD.has_error,
 unfinished_count=unfinished_count-(OLD.ended_at IS NULL),generation_count=generation_count-(OLD.span_type='generation'),
 function_count=function_count-(OLD.span_type='function') WHERE id=OLD.trace_id;
 UPDATE traces SET span_count=span_count+1,error_count=error_count+NEW.has_error,
 unfinished_count=unfinished_count+(NEW.ended_at IS NULL),generation_count=generation_count+(NEW.span_type='generation'),
 function_count=function_count+(NEW.span_type='function') WHERE id=NEW.trace_id;
END;
CREATE TABLE IF NOT EXISTS api_keys (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, prefix TEXT NOT NULL, hash TEXT NOT NULL UNIQUE,
 created_at INTEGER NOT NULL, expires_at INTEGER, last_used_at INTEGER, revoked_at INTEGER
);
CREATE TABLE IF NOT EXISTS sessions (
 hash TEXT PRIMARY KEY, github_id INTEGER NOT NULL, login TEXT NOT NULL, email TEXT NOT NULL, expires_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_expiry ON sessions(expires_at);
CREATE TABLE IF NOT EXISTS oauth_states (
 hash TEXT PRIMARY KEY, verifier TEXT NOT NULL, expires_at INTEGER NOT NULL
);
PRAGMA user_version=1;
