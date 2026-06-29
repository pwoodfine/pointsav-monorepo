'use strict';

(function () {
  function initDeck(deck) {
    var slides = Array.from(deck.querySelectorAll('.slide'));
    var total = slides.length;
    if (total === 0) return;

    var progress = deck.querySelector('.sd-progress');
    var prevBtn  = deck.querySelector('.sd-prev');
    var nextBtn  = deck.querySelector('.sd-next');
    var fsBtn    = deck.querySelector('.sd-fullscreen');
    var current  = 0;

    function updateControls() {
      if (progress) progress.textContent = (current + 1) + ' / ' + total;
      if (prevBtn)  prevBtn.setAttribute('aria-disabled', current === 0 ? 'true' : 'false');
      if (nextBtn)  nextBtn.setAttribute('aria-disabled', current === total - 1 ? 'true' : 'false');
    }

    function goto(n) {
      slides[current].classList.remove('active');
      slides[current].hidden = true;
      current = ((n % total) + total) % total;
      slides[current].hidden = false;
      slides[current].classList.add('active');
      updateControls();
      try { history.replaceState(null, '', '#slide-' + (current + 1)); } catch (_) {}
    }

    function prev() { if (current > 0) goto(current - 1); }
    function next() { if (current < total - 1) goto(current + 1); }

    if (prevBtn) prevBtn.addEventListener('click', prev);
    if (nextBtn) nextBtn.addEventListener('click', next);

    if (fsBtn) {
      fsBtn.addEventListener('click', function () {
        if (document.fullscreenElement === deck) {
          document.exitFullscreen().catch(function () {
            deck.classList.remove('sd-fullscreen--active');
          });
        } else if (deck.requestFullscreen) {
          deck.requestFullscreen().catch(function () {
            deck.classList.add('sd-fullscreen--active');
          });
        } else {
          // iOS Safari: CSS-only viewport overlay fallback
          deck.classList.toggle('sd-fullscreen--active');
        }
      });

      document.addEventListener('fullscreenchange', function () {
        if (document.fullscreenElement !== deck) {
          deck.classList.remove('sd-fullscreen--active');
        }
      });
    }

    deck.addEventListener('keydown', function (e) {
      switch (e.key) {
        case 'ArrowLeft':  prev(); break;
        case 'ArrowRight': next(); break;
        case 'f':
        case 'F':
          if (fsBtn) fsBtn.click();
          break;
        case 'Escape':
          if (deck.classList.contains('sd-fullscreen--active')) {
            deck.classList.remove('sd-fullscreen--active');
          }
          break;
      }
    });

    // Restore slide position from URL hash on load.
    var m = location.hash.match(/^#slide-(\d+)$/);
    if (m) {
      var n = parseInt(m[1], 10) - 1;
      if (n > 0 && n < total) goto(n);
    }

    // Make deck keyboard-focusable for arrow-key nav.
    if (!deck.hasAttribute('tabindex')) deck.setAttribute('tabindex', '0');

    updateControls();
  }

  document.querySelectorAll('.slide-deck').forEach(initDeck);
}());
