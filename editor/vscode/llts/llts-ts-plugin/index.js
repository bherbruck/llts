// TypeScript language service plugin for LLTS.
// Suppresses diagnostics that don't apply to LLTS projects.

function init() {
  function create(info) {
    const proxy = Object.create(null);
    const ls = info.languageService;

    // Copy all methods from the original language service.
    for (const k of Object.keys(ls)) {
      proxy[k] = (...args) => ls[k](...args);
    }

    // Filter out irrelevant diagnostics.
    function filterDiagnostics(diagnostics) {
      return diagnostics.filter((d) => {
        // 6133 = "'x' is declared but its value is never read"
        // 6196 = "'x' is declared but never used"
        if (d.code === 6133 || d.code === 6196) {
          const text = d.messageText;
          const msg = typeof text === "string" ? text : text.messageText;
          if (msg.includes("'main'")) {
            return false;
          }
        }
        return true;
      });
    }

    proxy.getSemanticDiagnostics = (fileName) => {
      return filterDiagnostics(ls.getSemanticDiagnostics(fileName));
    };

    proxy.getSuggestionDiagnostics = (fileName) => {
      return filterDiagnostics(ls.getSuggestionDiagnostics(fileName));
    };

    return proxy;
  }

  return { create };
}

module.exports = init;
