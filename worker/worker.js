function put_paste_ttl(key, val, ttl) {
  return PasteNS.put(key, value, { expirationTtl: ttl });
}

function test1(obj) {
  console.log(obj)
}

function test2(obj) {
  console.log(obj)
}

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
}
