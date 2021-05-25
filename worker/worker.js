addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request))
})

/**
 * Fetch and log a request
 * @param {Request} request
 */
async function handleRequest(request) {
  const { main } = wasm_bindgen;
  await wasm_bindgen(wasm)
  try {
    return await main(request);
  } catch (e) {
    return new Response(e.message || e || "unknown error", {
      status: 500
    })
  }
  // const greeting = greet()
  // return new Response(greeting, {status: 200})
}
