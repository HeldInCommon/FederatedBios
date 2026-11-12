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

  // A list of labels as one line ("Credit, Open"), or "None".
  function line(labels) {
    return labels.length ? labels.join(', ') : 'None';
  }

  // Held in Common Signals (decision record 0025): the withdraw choice if
  // ticked, then "Own terms" if anything is written. The same words the
  // server uses for this cell.
  function held() {
    var out = [];
    var withdraw = label('input[name=withdraw_after_training]:checked');
    if (withdraw) out.push(withdraw);
    var own = form.querySelector('textarea[name=own_terms]');
    if (own && own.value.trim()) out.push(own.getAttribute('data-label'));
    return line(out);
  }

  // Read every choice and update the summary.
  function update() {
    set('visibility', label('input[name=visibility]:checked'));
    set('license', label('input[name=license]:checked'));
    var signals = [].map.call(form.querySelectorAll('input[name=signals]:checked'), function (i) {
      return i.getAttribute('data-label');
    });
    set('signals', line(signals));
    set('held', held());
  }

  // `change` covers the choices; `input` covers typing in the own-terms box.
  form.addEventListener('change', update);
  form.addEventListener('input', update);
})();
