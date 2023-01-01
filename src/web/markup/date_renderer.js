document
    .querySelectorAll(".date")
    .forEach((element) => {
        const timestamp_str = element.getAttribute("data-unix-timestamp");

        if (timestamp_str == null) {
            console.warn(
                "element with date class does not have unix-timestamp attribute",
                element
            );
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

        element.textContent = new Date(timestamp).toLocaleString();
    });
