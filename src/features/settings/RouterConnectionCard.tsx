import { useCallback, useEffect, useState } from "react";

import StatusBadge from "../../components/status/StatusBadge";
import { Card, CardBody, CardHead } from "../../components/ui/Card";
import { RouterIcon } from "../../components/ui/icons";
import {
  clearRouterConnection,
  getRouterConnection,
  saveRouterConnection,
} from "../../lib/tauri/router";
import { isTauri } from "../../lib/tauri/environment";
import type { RouterConnectionReport } from "../../types/router";

const DEFAULT_HOST = "192.168.100.1";
const DEFAULT_PORT = 80;

/**
 * One labelled field with its input.
 *
 * The shared `field` visual (a caps label above a small value) is the same one
 * the read-only cards use, so editing never looks like a different app.
 */
function TextEntry({
  label,
  value,
  onEdit,
  placeholder,
  disabled,
  type = "text",
  autoCapitalize = false,
}: {
  label: string;
  value: string;
  onEdit: (value: string) => void;
  placeholder: string;
  disabled: boolean;
  type?: "text" | "password" | "number";
  autoCapitalize?: boolean;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="label-caps">{label}</span>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        autoCapitalize={autoCapitalize ? "characters" : "off"}
        autoCorrect="off"
        spellCheck={false}
        onChange={(event) => onEdit(event.target.value)}
        className="min-w-0 rounded-chip border border-hairline bg-surface-2 px-3 py-2 text-small text-ink outline-none transition-colors duration-[var(--motion)] placeholder:text-ink-faint focus:bg-surface-3 disabled:text-ink-faint"
      />
    </label>
  );
}

/**
 * The router connection, edited in place.
 *
 * A person who downloaded the app cannot be expected to launch it with
 * environment variables, so the address, port, and user name are fields like
 * any other settings form, and the password goes to the macOS Keychain rather
 * than to a file. The environment still wins while it is set, and the card
 * says so instead of letting the disabled form look like a malfunction.
 *
 * Honesty rules inherited from the rest of the app: the password is never
 * fetched back for display — the field stays empty with a note that one is
 * stored; a save without a password does not erase the stored one; and
 * "saved" never means "reading" — the report's `willRead` is what the badge
 * shows.
 */
export default function RouterConnectionCard() {
  const [report, setReport] = useState<RouterConnectionReport | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [host, setHost] = useState(DEFAULT_HOST);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  const [pending, setPending] = useState(false);
  const [note, setNote] = useState<{ tone: "ok" | "error"; text: string } | null>(
    null,
  );
  const [confirmingClear, setConfirmingClear] = useState(false);

  const apply = useCallback((next: RouterConnectionReport) => {
    setReport(next);
    // The form shows what is saved, and the password field is deliberately
    // blank either way: it is a write-only field.
    setHost(next.host ?? DEFAULT_HOST);
    setPort(String(next.port ?? DEFAULT_PORT));
    setUsername(next.username ?? "");
    setPassword("");
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      // In a browser the settings cannot be read or written; the card says so
      // rather than showing controls that would change nothing.
      setLoadError("Available in the FibreX Monitor app.");
      return;
    }
    getRouterConnection()
      .then(apply)
      .catch((error: unknown) => {
        setLoadError(String(error));
      });
  }, [apply]);

  const environmentOverride = report?.source === "environment";
  const disabled = report === null || pending || environmentOverride;

  const save = async () => {
    const parsedPort = Number(port);
    if (!Number.isInteger(parsedPort)) {
      setNote({ tone: "error", text: "The port must be a number." });
      return;
    }
    setPending(true);
    setNote(null);
    try {
      // An empty password keeps the stored one, which is what makes changing
      // the address without retyping the password the ordinary round trip.
      const passwordToWrite = password.trim().length > 0 ? password : null;
      const next = await saveRouterConnection({
        host,
        port: parsedPort,
        username,
        password: passwordToWrite,
      });
      apply(next);
      setNote({
        tone: next.willRead ? "ok" : "error",
        text: next.willRead
          ? "Saved. The first read happens within the next half a minute."
          : (next.passwordMissing
            ? "Saved, but no password was stored, so the router is not being read yet."
            : "Saved, but the router is not being read yet."),
      });
    } catch (error) {
      setNote({ tone: "error", text: String(error) });
    } finally {
      setPending(false);
    }
  };

  const clear = async () => {
    if (!confirmingClear) {
      // Two deliberate clicks rather than a native dialog, so an Undo of a
      // mistyped click is one more click and not a decision the user regrets
      // halfway through a popup.
      setConfirmingClear(true);
      return;
    }
    setPending(true);
    setNote(null);
    try {
      const next = await clearRouterConnection();
      apply(next);
      setConfirmingClear(false);
      setNote({ tone: "ok", text: "Removed. The router is no longer being read." });
    } catch (error) {
      setNote({ tone: "error", text: String(error) });
    } finally {
      setPending(false);
    }
  };

  const badge = environmentOverride
    ? {
        tone: "neutral" as const,
        label: "From environment",
      }
    : report === null
      ? { tone: "neutral" as const, label: "Unavailable" }
      : report.willRead
        ? { tone: "ok" as const, label: "Reading" }
        : report.passwordMissing
          ? { tone: "warn" as const, label: "Password missing" }
          : { tone: "neutral" as const, label: "Not configured" };

  return (
    <Card span={6}>
      <CardHead
        icon={<RouterIcon className="size-5" />}
        title="Router connection"
        sub="Read-only access to your router"
        end={<StatusBadge tone={badge.tone}>{badge.label}</StatusBadge>}
      />
      <CardBody>
        {loadError !== null ? (
          <p className="text-small text-ink-faint">{loadError}</p>
        ) : report === null ? (
          <p className="text-small text-ink-faint">Reading the stored settings…</p>
        ) : (
          <>
            <div className="grid grid-cols-2 gap-gap">
              <div className="col-span-2">
                <TextEntry
                  label="Address"
                  value={host}
                  onEdit={setHost}
                  placeholder={DEFAULT_HOST}
                  disabled={disabled}
                />
              </div>
              <TextEntry
                label="Port"
                value={port}
                onEdit={setPort}
                placeholder={String(DEFAULT_PORT)}
                disabled={disabled}
                type="number"
              />
              <TextEntry
                label="User name"
                value={username}
                onEdit={setUsername}
                placeholder="The router's admin user name"
                disabled={disabled}
                autoCapitalize
              />
              <div className="col-span-2">
                <TextEntry
                  label="Password"
                  value={password}
                  onEdit={setPassword}
                  placeholder={
                    report.hasPassword
                      ? "Stored — type a new one to replace it"
                      : "The router's admin password"
                  }
                  disabled={disabled}
                  type="password"
                />
              </div>
            </div>

            {environmentOverride && (
              <p className="mt-gap text-small text-ink-faint">
                The <code className="tnum">FIBREX_ROUTER_*</code> environment
                variables are in charge, so what is saved below is overridden
                while the app is started with them.
              </p>
            )}

            <div className="mt-stack flex items-center gap-row">
              <button
                type="button"
                onClick={() => void save()}
                disabled={disabled}
                className="inline-flex cursor-pointer items-center gap-gap rounded-control bg-ink px-4 py-2.5 text-small font-semibold text-canvas transition-opacity duration-[var(--motion)] hover:opacity-80 disabled:cursor-default disabled:opacity-40"
              >
                {pending ? "Saving…" : "Save connection"}
              </button>
              <button
                type="button"
                onClick={() => void clear()}
                disabled={report.source === null || pending}
                className="inline-flex cursor-pointer items-center gap-gap rounded-control bg-surface-2 px-4 py-2.5 text-small font-semibold text-ink transition-colors duration-[var(--motion)] hover:bg-surface-3 disabled:cursor-default disabled:opacity-40"
              >
                {confirmingClear ? "Confirm removal" : "Remove"}
              </button>
            </div>

            <p
              className={`mt-gap text-small ${
                note?.tone === "error" ? "text-bad" : "text-ink-faint"
              }`}
              role="status"
            >
              {note === null
                ? "Read-only: the app never sends a configuration change to the router."
                : note.text}
            </p>
          </>
        )}
      </CardBody>
    </Card>
  );
}
