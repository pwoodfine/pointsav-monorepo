// P0.2 — click-to-copy token names on the /tokens gallery page.
(function () {
  document.addEventListener('click', function (e) {
    var el = e.target.closest('.tg-name');
    if (!el) return;
    var text = el.getAttribute('data-copy') || el.textContent;
    navigator.clipboard.writeText(text).then(function () {
      var prev = el.textContent;
      el.textContent = 'copied!';
      setTimeout(function () { el.textContent = prev; }, 900);
    });
  });
})();
