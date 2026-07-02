// app-mediakit-marketing-2 — mobile drawer toggle.
// Progressive enhancement: the drawer is pre-rendered HTML (works
// no-JS-degraded per app.css's `html:not(.js) .m-drawer { display: none }`);
// this only wires the open/close interaction.
(function () {
  document.documentElement.classList.add("js");

  var burger = document.querySelector("[data-m-drawer-toggle]");
  var drawer = document.getElementById("m-drawer");
  var scrim = document.querySelector("[data-m-drawer-scrim]");
  var closeButtons = document.querySelectorAll("[data-m-drawer-toggle]");
  if (!drawer || !scrim) return;

  function openDrawer() {
    drawer.hidden = false;
    // Force layout so the transform transition runs from translateX(-100%).
    void drawer.offsetWidth;
    drawer.setAttribute("data-open", "");
    scrim.setAttribute("data-open", "");
    if (burger) burger.setAttribute("aria-expanded", "true");
    document.body.style.overflow = "hidden";
    var firstLink = drawer.querySelector("a, button");
    if (firstLink) firstLink.focus();
  }

  function closeDrawer() {
    drawer.removeAttribute("data-open");
    scrim.removeAttribute("data-open");
    if (burger) burger.setAttribute("aria-expanded", "false");
    document.body.style.overflow = "";
    window.setTimeout(function () {
      if (!drawer.hasAttribute("data-open")) drawer.hidden = true;
    }, 300); // matches --m-dur-slow
    if (burger) burger.focus();
  }

  closeButtons.forEach(function (el) {
    el.addEventListener("click", function () {
      if (drawer.hasAttribute("data-open")) {
        closeDrawer();
      } else {
        openDrawer();
      }
    });
  });

  scrim.addEventListener("click", closeDrawer);

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape" && drawer.hasAttribute("data-open")) {
      closeDrawer();
    }
  });
})();
