// Native scroll sync for the code editor panes.
//
// Delegated, capture-phase scroll listener that mirrors an editor's scroll
// position onto its line-number gutter and syntax-highlight overlay. This runs
// natively so scroll tracking has no Rust/async round-trip overhead. The pane is
// identified via the `data-editor-pane` attribute because the CSS module class
// names are hashed at build time.
document.addEventListener('scroll', function (e) {
    var paneId = e.target.dataset && e.target.dataset.editorPane;
    if (!paneId) return;

    var editor = e.target;
    var lineNumbers = document.getElementById('line-numbers-' + paneId);
    var syntax = document.getElementById('syntax-' + paneId);

    if (lineNumbers) {
        lineNumbers.scrollTop = editor.scrollTop;
    }
    if (syntax) {
        syntax.style.transform = 'translateY(-' + editor.scrollTop + 'px)';
    }
}, true);
