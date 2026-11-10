// Profile-picture picker: resize + square-crop the chosen image on the user's
// own device, then stash it as a small JPEG data URI in a hidden field. No
// upload of the original file, no server-side image library — the data URI
// rides along in the normal form POST and becomes part of the signed,
// content-addressed profile.
(function () {
  var photo = document.getElementById('photo');
  var hidden = document.getElementById('avatar');
  var preview = document.getElementById('avatar-preview');
  var clear = document.getElementById('avatar-clear');
  if (!photo || !hidden) return;

  var SIZE = 320; // output square, px

  function setPreview(dataUrl) {
    hidden.value = dataUrl || '';
    if (preview) {
      if (dataUrl) { preview.src = dataUrl; preview.classList.remove('empty'); }
      else { preview.removeAttribute('src'); preview.classList.add('empty'); }
    }
    if (clear) clear.style.display = dataUrl ? '' : 'none';
  }

  photo.addEventListener('change', function () {
    var file = photo.files && photo.files[0];
    if (!file) return;
    var reader = new FileReader();
    reader.onload = function (e) {
      var img = new Image();
      img.onload = function () {
        var canvas = document.createElement('canvas');
        canvas.width = SIZE; canvas.height = SIZE;
        var ctx = canvas.getContext('2d');
        var s = Math.min(img.width, img.height);
        var sx = (img.width - s) / 2, sy = (img.height - s) / 2;
        ctx.drawImage(img, sx, sy, s, s, 0, 0, SIZE, SIZE);
        try { setPreview(canvas.toDataURL('image/jpeg', 0.82)); } catch (err) {}
      };
      img.src = e.target.result;
    };
    reader.readAsDataURL(file);
  });

  if (clear) {
    clear.addEventListener('click', function (ev) {
      ev.preventDefault(); photo.value = ''; setPreview('');
    });
  }
})();
