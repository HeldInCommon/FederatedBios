// Keeps "What travels with every copy" in step with the Sharing and rights
// choices as someone changes them. Each choice carries a data-label; the
// summary cells carry data-summary. Without this script the form works the
// same and the summary shows what was chosen when the page loaded.
(function () {
  var form = document.querySelector('.rights-form');
  if (!form) return;

  // The plain-words label of the first input matching `selector`, if any.
  function label(selector) {
    var input = form.querySelector(selector);
    return input ? input.getAttribute('data-label') : null;
  }

  // Put `text` in the summary cell for `key`. Empty text leaves it as it was.
  function set(key, text) {
    var cell = form.querySelector('[data-summary="' + key + '"]');
    if (cell && text) cell.textContent = text;
  }

  form.addEventListener('change', function () {
    set('visibility', label('input[name=visibility]:checked'));
    set('license', label('input[name=license]:checked'));
    var signals = [].map.call(form.querySelectorAll('input[name=signals]:checked'), function (i) {
      return i.getAttribute('data-label');
    });
    set('signals', signals.length ? signals.join(', ') : 'None');
  });
})();
