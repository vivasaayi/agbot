// Portal session persistence: bearer token + identity in localStorage.

const TOKEN_KEY = "agbot_portal_token";
const IDENTITY_KEY = "agbot_portal_identity";

export function getToken() {
  try {
    return window.localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export function isLoggedIn() {
  return Boolean(getToken());
}

/** Persist a successful portal login response. */
export function storeSession(loginResponse) {
  try {
    window.localStorage.setItem(TOKEN_KEY, loginResponse.token);
    window.localStorage.setItem(
      IDENTITY_KEY,
      JSON.stringify({
        account_id: loginResponse.account_id,
        org_id: loginResponse.org_id,
        party_type: loginResponse.party_type,
        expires_at: loginResponse.expires_at,
      }),
    );
  } catch {
    // Private-mode storage failures degrade to a per-page session.
  }
}

export function getIdentity() {
  try {
    const raw = window.localStorage.getItem(IDENTITY_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

export function clearSession() {
  try {
    window.localStorage.removeItem(TOKEN_KEY);
    window.localStorage.removeItem(IDENTITY_KEY);
  } catch {
    // Ignore storage failures; the in-memory state is already gone.
  }
}
