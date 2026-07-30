// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Closes .site-nav__dropdown ("Product lines" / "More") on link-selection and on
// outside-click — native <details>/<summary> can't do either on its own. Ported
// verbatim from the approved v3 mockup's _chrome-snippet.html (one narrowly scoped
// JS exception in an otherwise no-JS mockup), matching every hyperscaler nav
// examined (Carbon, Primer's ActionMenu, Stripe docs).
(function () {
  document.querySelectorAll(".site-nav__dropdown").forEach(function (dropdown) {
    dropdown.addEventListener("click", function (event) {
      if (event.target.closest("a")) dropdown.open = false;
    });
  });
  document.addEventListener("click", function (event) {
    document.querySelectorAll(".site-nav__dropdown[open]").forEach(function (dropdown) {
      if (!dropdown.contains(event.target)) dropdown.open = false;
    });
  });
})();
