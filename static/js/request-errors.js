// htmx normally skips error responses. Render only the error fragments explicitly
// marked by the app, using their HX-Retarget and HX-Reswap response headers.
document.addEventListener('htmx:beforeSwap', (event) => {
    if (event.detail.xhr.getResponseHeader('X-App-Error') === 'true') {
        event.detail.shouldSwap = true;
    }
});

document.addEventListener('htmx:beforeRequest', () => {
    document.getElementById('request-error')?.replaceChildren();
});

function showRequestError(message) {
    const target = document.getElementById('request-error');
    if (target) target.textContent = message;
}

document.addEventListener('htmx:sendError', () => {
    showRequestError('Could not reach the server. Your entries are preserved. Please try again.');
});

document.addEventListener('htmx:timeout', () => {
    showRequestError('The request timed out. Please check the list before retrying.');
});

document.addEventListener('htmx:responseError', (event) => {
    if (event.detail.xhr.getResponseHeader('X-App-Error') !== 'true') {
        showRequestError('The request failed. Please try again.');
    }
});
