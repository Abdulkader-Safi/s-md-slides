// Live reload for `s-md-slides serve`: on a `reload` event, reload the page. The
// runtime keeps the current slide in the URL hash, so the reload lands on it.
new EventSource("/events").addEventListener("reload", () => location.reload());
