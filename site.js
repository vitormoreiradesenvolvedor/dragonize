(function () {
  var root = document.documentElement;
  function setLang(l) {
    root.setAttribute('data-lang', l);
    root.setAttribute('lang', l === 'pt' ? 'pt-BR' : 'en');
    document.querySelectorAll('.lang button').forEach(function (b) {
      b.classList.toggle('on', b.dataset.set === l);
    });
    try { localStorage.setItem('dragonize-lang', l); } catch (e) {}
  }
  var saved = null;
  try { saved = localStorage.getItem('dragonize-lang'); } catch (e) {}
  setLang(saved || ((navigator.language || '').toLowerCase().indexOf('pt') === 0 ? 'pt' : 'en'));
  document.querySelectorAll('.lang button').forEach(function (b) {
    b.addEventListener('click', function () { setLang(b.dataset.set); });
  });

  var img = document.getElementById('shotImg'), cap = document.getElementById('shotCap');
  document.querySelectorAll('#shotTabs button').forEach(function (b) {
    b.addEventListener('click', function () {
      document.querySelectorAll('#shotTabs button').forEach(function (x) { x.classList.remove('on'); });
      b.classList.add('on');
      img.src = 'img/' + b.dataset.img;
      cap.textContent = b.dataset.cap;
    });
  });

  document.querySelectorAll('.code .copy').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var text = btn.parentNode.firstChild.textContent.trim();
      navigator.clipboard.writeText(text).then(function () {
        btn.textContent = 'ok';
        setTimeout(function () { btn.textContent = 'copy'; }, 1200);
      });
    });
  });
})();
