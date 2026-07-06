// AGBot Farm portal shell: hash router, view mounting, service worker
// registration, install prompt, and the offline banner.

import { OFFLINE_EVENT, logout, notificationsSummary } from "./api.js";
import { clearSession, isLoggedIn } from "./auth.js";
import { renderLogin } from "./views/login.js";
import { renderHome } from "./views/home.js";
import { renderReports } from "./views/reports.js";
import { renderFieldStub } from "./views/field.js";

const view = document.getElementById("view");
const tabBar = document.getElementById("tab-bar");
const logoutButton = document.getElementById("logout-button");
const installButton = document.getElementById("install-button");
const notificationBadge = document.getElementById("notification-badge");
const offlineBanner = document.getElementById("offline-banner");

// --- Router -----------------------------------------------------------------

/** Parse "#/field/f-1" into {name: "field", param: "f-1"}. */
function parseRoute() {
  const hash = window.location.hash || "#/home";
  const segments = hash.replace(/^#\//, "").split("/").filter(Boolean);
  return { name: segments[0] || "home", param: segments[1] || null };
}

async function mountRoute() {
  const route = parseRoute();

  if (!isLoggedIn() && route.name !== "login") {
    window.location.hash = "#/login";
    return;
  }
  if (isLoggedIn() && route.name === "login") {
    window.location.hash = "#/home";
    return;
  }

  const loggedIn = isLoggedIn();
  tabBar.hidden = !loggedIn;
  logoutButton.hidden = !loggedIn;

  view.innerHTML = "";
  switch (route.name) {
    case "login":
      renderLogin(view);
      break;
    case "reports":
      renderReports(view);
      break;
    case "field":
      // F-B7 replaces this stub with the full field-detail view.
      renderFieldStub(view, route.param);
      break;
    case "home":
    default:
      renderHome(view);
      break;
  }

  for (const tab of tabBar.querySelectorAll(".tab")) {
    tab.classList.toggle("active", tab.dataset.tab === route.name);
  }

  if (loggedIn) {
    refreshNotificationBadge();
  } else {
    notificationBadge.hidden = true;
  }
}

async function refreshNotificationBadge() {
  try {
    const summary = await notificationsSummary();
    const count =
      (summary.unread_reports || 0) + (summary.alerts_last_7d || 0);
    notificationBadge.textContent = String(count);
    notificationBadge.hidden = count === 0;
  } catch {
    // Badge is best-effort; offline or auth errors already surface elsewhere.
  }
}

window.addEventListener("hashchange", mountRoute);

// --- Logout -----------------------------------------------------------------

logoutButton.addEventListener("click", async () => {
  try {
    await logout();
  } catch {
    // Session is cleared locally regardless of server reachability.
  }
  clearSession();
  window.location.hash = "#/login";
});

// --- Offline banner ----------------------------------------------------------

function setOffline(offline) {
  offlineBanner.hidden = !offline;
}

window.addEventListener("online", () => setOffline(false));
window.addEventListener("offline", () => setOffline(true));
window.addEventListener(OFFLINE_EVENT, () => setOffline(true));
setOffline(!navigator.onLine);

// --- Install prompt ----------------------------------------------------------

let deferredInstallPrompt = null;

window.addEventListener("beforeinstallprompt", (event) => {
  event.preventDefault();
  deferredInstallPrompt = event;
  installButton.hidden = false;
});

installButton.addEventListener("click", async () => {
  if (!deferredInstallPrompt) {
    return;
  }
  deferredInstallPrompt.prompt();
  await deferredInstallPrompt.userChoice;
  deferredInstallPrompt = null;
  installButton.hidden = true;
});

window.addEventListener("appinstalled", () => {
  deferredInstallPrompt = null;
  installButton.hidden = true;
});

// --- Service worker ----------------------------------------------------------

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/portal/sw.js").catch(() => {
    // The app still works fully online without the service worker.
  });
}

mountRoute();
