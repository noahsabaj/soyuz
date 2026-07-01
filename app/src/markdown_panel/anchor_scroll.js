// Smooth-scroll in-page anchor links (href="#heading-id") inside the docs panel.
//
// Loaded via `document::Script { src: ... }`, which Dioxus deduplicates by `src`,
// so this installs exactly ONE delegated listener on `document` no matter how many
// markdown tabs mount or re-render. That replaces the previous inline
// `<script dangerous_inner_html>` that re-added a listener on every render.
//
// The listener is scoped to the docs content via the `data-docs-anchor-scroll`
// attribute (a stable hook that is never subject to CSS-module class hashing) and
// only intercepts links whose target actually exists, leaving unrelated "#" links
// untouched.
(function () {
  if (window.__soyuzAnchorScrollInstalled) {
    return;
  }
  window.__soyuzAnchorScrollInstalled = true;

  document.addEventListener(
    "click",
    function (e) {
      var link = e.target && e.target.closest ? e.target.closest("a") : null;
      if (!link || !link.closest("[data-docs-anchor-scroll]")) {
        return;
      }

      var href = link.getAttribute("href");
      if (!href || href.charAt(0) !== "#") {
        return;
      }

      var target = document.getElementById(href.slice(1));
      if (!target) {
        return;
      }

      e.preventDefault();
      e.stopPropagation();
      target.scrollIntoView({ behavior: "smooth", block: "start" });
    },
    true
  );
})();
