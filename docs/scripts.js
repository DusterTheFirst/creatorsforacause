import { createApp } from "https://unpkg.com/vue@3/dist/vue.esm-browser.js";


/**
 * Create a uri to the correct API endpoint
 *
 * @param {string} path
 */
function api_endpoint(path) {
    if (path.charAt(0) !== "/") {
        console.warn(
            "path provided to api_endpoint does not begin with /",
            path
        );
        path = `/${path}`;
    }

    // Localhost check
    if (
        document.location.hostname === "localhost" ||
        document.location.hostname.indexOf("127.0.0") !== -1
    ) {
        return `http://localhost:8080${path}`;
    }

    return `https://creatorsforacause.fly.dev${path}`;
}

document.addEventListener("DOMContentLoaded", async () => {
    const [fundraiser, streams] = await Promise.all([
        get_fundraiser_data(),
        get_stream_data(),
    ]);

    const fundraiser_elements = {
        /** @type {HTMLSpanElement} */
        funds: document.getElementById("fundraiser.funds"),
        /** @type {HTMLSpanElement} */
        currency: document.getElementById("fundraiser.currency"),
    };

    fundraiser_elements.funds.innerText = fundraiser.amountRaised.toString(10);
    fundraiser_elements.currency.innerText = fundraiser.causeCurrency;

    create_stream_cards(streams.twitch.streams);
});

/**
 * @param {{[x: string]: LiveStreamDetails | null}} streams
 */
function create_stream_cards(streams) {
    const streams_template = /** @type {HTMLTemplateElement} */ (
        document.getElementById("streams.template")
    );

    for (let streamer in streams) {
        const stream_card = /** @type {HTMLDivElement} */ (
            streams_template.content.firstElementChild.cloneNode(true)
        );

        const stream_card_slots = {
            /** @type {HTMLSlotElement} */
            streamer: stream_card.querySelector("slot[name='streamer']"),

            /** @type {HTMLSlotElement} */
            title: stream_card.querySelector("slot[name='title']"),

            /** @type {HTMLSlotElement} */
            start_time: stream_card.querySelector("slot[name='start-time']"),

            /** @type {HTMLSlotElement} */
            viewers: stream_card.querySelector("slot[name='viewers']"),
        };

        stream_card_slots.streamer.innerText = streamer;

        const info = streams[streamer];

        if (info !== null) {
            stream_card_slots.title.innerText = info.title;
            stream_card_slots.start_time.innerText = info.start_time;
            stream_card_slots.viewers.innerText = info.viewers;
        }

        document.getElementById("streams.twitch").appendChild(stream_card);
    }
}

/**
 * @typedef Fundraiser
 * @property {number} amountRaised
 * @property {string} causeCurrency
 */

/**
 * @returns {Promise<Fundraiser>}
 */
async function get_fundraiser_data() {
    let response = await fetch(api_endpoint("/fundraiser"));

    if (!response.ok) {
        throw Error("bad response from fundraiser endpoint");
    }

    let body = /** @type {{data: Fundraiser}} */ (await response.json());

    return body.data;
}

/**
 * @typedef LiveStreamList
 * @property {string} updated
 * @property {{[x: string]: LiveStreamDetails | null }} streams
 */

/**
 * @typedef LiveStreamDetails
 * @property {string} href
 * @property {string} title
 * @property {string} start_time
 * @property {string} viewers
 */

/**
 * @returns {Promise<{youtube: LiveStreamList, twitch: LiveStreamList}>}
 */
async function get_stream_data() {
    let response = await fetch(api_endpoint("/streams"));

    if (!response.ok) {
        throw Error("bad response from streams endpoint");
    }

    return await response.json();
}
