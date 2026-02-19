// Language switcher for Synaptic documentation
// Injects an EN/ZH toggle into the mdBook menu bar.
(function () {
  "use strict";

  var currentPath = window.location.pathname;
  var isZh = currentPath.indexOf("/zh/") !== -1;

  var targetPath;
  var label;
  if (isZh) {
    // Switch to English — strip /zh/ prefix
    targetPath = currentPath.replace(/\/zh\//, "/");
    label = "EN";
  } else {
    // Switch to Chinese — insert /zh/ after base path
    targetPath = currentPath.replace(/^(\/synapse\/)/, "$1zh/");
    // Fallback: if no /synapse/ prefix (local dev), just prepend /zh
    if (targetPath === currentPath) {
      targetPath = "/zh" + currentPath;
    }
    label = "ZH";
  }

  function insertSwitcher() {
    var rightButtons = document.querySelector(".right-buttons");
    if (!rightButtons) return;

    var link = document.createElement("a");
    link.href = targetPath;
    link.className = "lang-switcher";
    link.title = isZh ? "Switch to English" : "切换到中文";
    link.textContent = label;

    rightButtons.insertBefore(link, rightButtons.firstChild);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", insertSwitcher);
  } else {
    insertSwitcher();
  }
})();
