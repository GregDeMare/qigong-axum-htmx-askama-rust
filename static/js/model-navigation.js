// Refresh with the current filter controls after a successful mutation. Using the
// original request element also works when its row/form has already been swapped.
document.addEventListener('htmx:afterRequest', (event) => {
    const { successful, requestConfig } = event.detail;
    if (successful && requestConfig?.elt?.hasAttribute('data-model-mutation')) {
        const rows = document.getElementById('rows');
        if (rows) htmx.trigger(rows, 'modelsChanged');
    }
});
