-- Search only normalized identifiers, workflow/group, model and error fields.
-- Keeping these apart from payloads avoids reading gigabytes for an empty result.
CREATE VIRTUAL TABLE trace_search USING fts5(trace_id,workflow_name,group_id,models,errors,tokenize='trigram');
INSERT INTO trace_search(rowid,trace_id,workflow_name,group_id,models,errors)
 SELECT t.rowid,t.id,t.workflow_name,t.group_id,
 (SELECT GROUP_CONCAT(DISTINCT model) FROM spans WHERE trace_id=t.id),
 (SELECT GROUP_CONCAT(DISTINCT NULLIF(error_text,'')) FROM spans WHERE trace_id=t.id)
 FROM traces t;
CREATE TRIGGER traces_search_delete AFTER DELETE ON traces BEGIN
 DELETE FROM trace_search WHERE rowid=OLD.rowid;
END;
PRAGMA user_version=2;
