// Field detail STUB (batch F-B7 replaces this module with the real view:
// overview, map with Leaflet product layers, recommendations, activities).

export function renderFieldStub(container, fieldId) {
  const section = document.createElement("section");
  section.className = "view field-stub";

  const back = document.createElement("a");
  back.href = "#/home";
  back.className = "back-link";
  back.textContent = "← Back to fields";

  const heading = document.createElement("h2");
  heading.textContent = fieldId ? `Field ${fieldId}` : "Field";

  const message = document.createElement("p");
  message.className = "muted";
  message.textContent = "Field detail coming soon";

  section.append(back, heading, message);
  container.appendChild(section);
}
