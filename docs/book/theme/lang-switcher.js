// Language switcher for Synaptic documentation
// EN docs at /synaptic/  —  ZH docs at /synaptic/zh/
(function () {
  "use strict";

  var path = window.location.pathname;
  var isZh = /\/zh\//.test(path);

  function insertSwitcher() {
    var rightButtons = document.querySelector(".right-buttons");
    if (!rightButtons) return;

    var container = document.createElement("div");
    container.className = "lang-switcher";

    var enLink = document.createElement("a");
    enLink.textContent = "EN";
    enLink.title = "English";

    var sep = document.createElement("span");
    sep.className = "lang-sep";
    sep.textContent = "|";

    var zhLink = document.createElement("a");
    zhLink.textContent = "\u4E2D\u6587";
    zhLink.title = "\u5207\u6362\u5230\u4E2D\u6587";

    if (isZh) {
      // Currently Chinese → EN link removes /zh/
      enLink.href = path.replace(/\/zh\//, "/");
      enLink.className = "";
      zhLink.href = "#";
      zhLink.className = "lang-active";
      zhLink.onclick = function (e) { e.preventDefault(); };
    } else {
      // Currently English → ZH link inserts /zh/ after base path
      // e.g. /synaptic/foo.html → /synaptic/zh/foo.html
      var match = path.match(/^(\/[^/]+\/)(.*)/);
      if (match) {
        zhLink.href = match[1] + "zh/" + match[2];
      } else {
        zhLink.href = "/zh" + path;
      }
      zhLink.className = "";
      enLink.href = "#";
      enLink.className = "lang-active";
      enLink.onclick = function (e) { e.preventDefault(); };
    }

    container.appendChild(enLink);
    container.appendChild(sep);
    container.appendChild(zhLink);
    rightButtons.insertBefore(container, rightButtons.firstChild);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", insertSwitcher);
  } else {
    insertSwitcher();
  }
})();
