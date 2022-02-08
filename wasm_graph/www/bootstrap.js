init();

async function init() {
    const [{Chart, default: init}, {main, setup}] = await Promise.all([
        import("./wasm_graph.js"),
        import("./index.js"),
    ]);
    await init();
    setup(Chart);
    main();
}
