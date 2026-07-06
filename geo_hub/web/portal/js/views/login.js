// Login view: exchange an access code (agb-...) for a bearer session.

import { login } from "../api.js";
import { storeSession } from "../auth.js";

export function renderLogin(container) {
  const section = document.createElement("section");
  section.className = "view login-view";
  section.innerHTML = `
    <h2>Welcome</h2>
    <p class="muted">Enter the access code from your agronomist to see your
    fields, reports, and alerts.</p>
    <form id="login-form" novalidate>
      <label for="access-code">Access code</label>
      <input id="access-code" name="access-code" type="text"
             inputmode="text" autocomplete="one-time-code"
             placeholder="agb-..." required autofocus>
      <p id="login-error" class="error-text" hidden></p>
      <button type="submit" id="login-submit" class="primary-button">
        Sign in
      </button>
    </form>
  `;
  container.appendChild(section);

  const form = section.querySelector("#login-form");
  const input = section.querySelector("#access-code");
  const errorText = section.querySelector("#login-error");
  const submit = section.querySelector("#login-submit");

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const accessCode = input.value.trim();
    errorText.hidden = true;
    if (!accessCode) {
      errorText.textContent = "Enter your access code.";
      errorText.hidden = false;
      return;
    }

    submit.disabled = true;
    submit.textContent = "Signing in…";
    try {
      const session = await login(accessCode);
      storeSession(session);
      window.location.hash = "#/home";
    } catch (error) {
      errorText.textContent = error.offline
        ? "You are offline. Connect to the internet to sign in."
        : "That access code was not accepted. Check it and try again.";
      errorText.hidden = false;
    } finally {
      submit.disabled = false;
      submit.textContent = "Sign in";
    }
  });
}
