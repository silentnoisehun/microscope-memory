// Microscope Memory - Cloudflare Worker Proxy
export default {
  async fetch(request) {
    const MICROSCOPE = "http://your-server:6060/v1";
    const url = new URL(request.url);
    const target = MICROSCOPE + url.pathname + url.search;
    
    if (request.method === "GET") {
      const res = await fetch(target);
      return new Response(await res.text(), {
        headers: { "Access-Control-Allow-Origin": "*", "Content-Type": "application/json" }
      });
    }
    
    if (request.method === "POST") {
      const body = await request.json();
      const res = await fetch(target, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body)
      });
      return new Response(await res.text(), {
        headers: { "Access-Control-Allow-Origin": "*" }
      });
    }

    return new Response("Microscope Memory Worker", { status: 200 });
  }
}
