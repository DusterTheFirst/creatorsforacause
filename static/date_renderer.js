/**
 *
 * @param {HTMLElement} element
 * @returns
 */
function update_time(element) {
    const timestamp_str = element.getAttribute("data-unix-timestamp");

    if (!element.classList.contains("date")) {
        console.warn("data-unix-timestamp was changed on a non-date element");
        return;
    }

    const timestamp = parseInt(timestamp_str, 10);

    if (Number.isNaN(timestamp)) {
        console.warn(
            "timestamp provided in unix-timestamp is parsed as NaN",
            element
        );
        return;
    }

    const date = new Date(timestamp);

    element.textContent = Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "full",
    }).format(date);
    element.title = Intl.DateTimeFormat(undefined, {
        dateStyle: "full",
        timeStyle: "full",
        timeZone: "UTC",
    }).format(date);
}

/**
 *
 * @param {MutationRecord[]} mutations
 * @param {MutationObserver} observer
 */
function mutation_callback(mutations, observer) {
    for (let mutation of mutations) {
        const element = mutation.target;
        if (element instanceof HTMLElement) {
            if (
                mutation.type === "attributes" &&
                mutation.attributeName === "data-unix-timestamp"
            ) {
                update_time(element);
            } else if (mutation.type === "childList") {
                element.querySelectorAll(".date").forEach(update_time);
            }
        }
    }
}

let mutation_observer = new MutationObserver(mutation_callback);
mutation_observer.observe(document, {
    attributes: true,
    attributeFilter: ["data-unix-timestamp"],
    subtree: true,
    childList: true,
});
