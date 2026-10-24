/**
 * The router connection the user edits in Settings.
 *
 * These are contract types, not data model types: they mirror what the Rust
 * commands return, and `null` means "not set" rather than zero. The password
 * itself never crosses this contract — only whether one is stored.
 */

/** Where the effective credentials come from, named as the card shows them. */
export type RouterCredentialSource = "environment" | "appSettings";

/** The state of the router connection, without ever carrying the password. */
export interface RouterConnectionReport {
  /** Which source is in charge, or null when nothing is configured at all. */
  source: RouterCredentialSource | null;
  host: string | null;
  port: number | null;
  username: string | null;
  hasPassword: boolean;
  /** Router reads will actually be attempted with what is configured. */
  willRead: boolean;
  /** A configuration is saved but the password was never stored. */
  passwordMissing: boolean;
}

/** What a save sends. A null or empty password keeps the stored one. */
export interface RouterConnectionSave {
  host: string;
  port: number;
  username: string;
  password?: string | null;
}
