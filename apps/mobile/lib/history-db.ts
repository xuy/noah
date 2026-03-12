import * as SQLite from "expo-sqlite";

export interface AnalysisRecord {
  id: number;
  photo_uri: string;
  analysis: string;
  model: string;
  created_at: string;
}

let db: SQLite.SQLiteDatabase | null = null;

async function getDb(): Promise<SQLite.SQLiteDatabase> {
  if (!db) {
    db = await SQLite.openDatabaseAsync("noah-history.db");
    await db.execAsync(`
      CREATE TABLE IF NOT EXISTS analyses (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        photo_uri TEXT NOT NULL,
        analysis TEXT NOT NULL,
        model TEXT NOT NULL DEFAULT '',
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
      );
    `);
  }
  return db;
}

export async function saveAnalysis(
  photoUri: string,
  analysis: string,
  model: string,
): Promise<number> {
  const conn = await getDb();
  const result = await conn.runAsync(
    "INSERT INTO analyses (photo_uri, analysis, model) VALUES (?, ?, ?)",
    [photoUri, analysis, model],
  );
  return result.lastInsertRowId;
}

export async function listAnalyses(): Promise<AnalysisRecord[]> {
  const conn = await getDb();
  return await conn.getAllAsync<AnalysisRecord>(
    "SELECT * FROM analyses ORDER BY created_at DESC",
  );
}

export async function getAnalysis(id: number): Promise<AnalysisRecord | null> {
  const conn = await getDb();
  return await conn.getFirstAsync<AnalysisRecord>(
    "SELECT * FROM analyses WHERE id = ?",
    [id],
  );
}

export async function deleteAnalysis(id: number): Promise<void> {
  const conn = await getDb();
  await conn.runAsync("DELETE FROM analyses WHERE id = ?", [id]);
}
