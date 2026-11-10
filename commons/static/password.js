// Show/hide button for passphrase boxes.
//
// Adds a small eye button inside every <input type="password"> on the page.
// Pressing it shows what you've typed (so you can check it before signing up
// or downloading a backup); pressing again hides it. Labelled for screen
// readers and keyboard-usable. Loaded on every page; does nothing where there
// is no passphrase box.
(function () {
  var EYE = '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7S1 12 1 12z"/><circle cx="12" cy="12" r="3"/></svg>';
  var EYE_OFF = '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2"><path d="M17.9 17.9A10.9 10.9 0 0 1 12 19c-7 0-11-7-11-7a20 20 0 0 1 5.1-5.9M9.9 4.2A10.5 10.5 0 0 1 12 4c7 0 11 7 11 7a20 20 0 0 1-2.2 3.2M1 1l22 22"/><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2"/></svg>';

  document.querySelectorAll('input[type="password"]').forEach(function (input) {
    var wrap = document.createElement('span');
    wrap.className = 'pw-wrap';
    input.parentNode.insertBefore(wrap, input);
    wrap.appendChild(input);

    var button = document.createElement('button');
    button.type = 'button';
    button.className = 'pw-toggle';
    button.setAttribute('aria-label', 'Show passphrase');
    button.setAttribute('aria-pressed', 'false');
    button.innerHTML = EYE;
    button.addEventListener('click', function () {
      var show = input.type === 'password';
      input.type = show ? 'text' : 'password';
      button.setAttribute('aria-pressed', show ? 'true' : 'false');
      button.setAttribute('aria-label', show ? 'Hide passphrase' : 'Show passphrase');
      button.innerHTML = show ? EYE_OFF : EYE;
      input.focus();
    });
    wrap.appendChild(button);
  });
})();
