import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";

import * as schema from "./schema";

const connectionString = process.env.DATABASE_URL;

if (!connectionString) {
  throw new Error("DATABASE_URL is required");
}

type GlobalWithPostgres = typeof globalThis & {
  __agentTracesSql?: postgres.Sql;
};

const globalForPostgres = globalThis as GlobalWithPostgres;

export const sqlClient =
  globalForPostgres.__agentTracesSql ??
  postgres(connectionString, {
    max: 10,
    prepare: false,
  });

if (process.env.NODE_ENV !== "production") {
  globalForPostgres.__agentTracesSql = sqlClient;
}

export const db = drizzle(sqlClient, { schema });
export { schema };
