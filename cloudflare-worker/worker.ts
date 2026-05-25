interface D1Database {
  prepare(query: string): { bind(...values: unknown[]): { all<T>(): Promise<{ results: T[] }>; first<T>(): Promise<T | null> } };
}

const PLATFORM_MAP: Record<string, string> = {
  'mac-arm': 'darwin-aarch64',
  'mac-intel': 'darwin-x86_64',
  'windows': 'windows-x86_64',
  'linux': 'linux-x86_64',
};

const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type',
};

export default {
  async fetch(request: Request, env: { DB: D1Database; BUCKET: R2Bucket }): Promise<Response> {
    if (request.method === 'OPTIONS') return new Response(null, { headers: CORS });
    const url = new URL(request.url);

    // Serve files from R2
    if (request.method === 'GET' && url.pathname.startsWith('/releases/')) {
      const key = decodeURIComponent(url.pathname.replace('/releases/', ''));
      const obj = await env.BUCKET.get(key);
      if (!obj) return new Response(`Not found: ${key}`, { status: 404, headers: CORS });
      return new Response(obj.body, {
        headers: {
          'Content-Type': obj.httpMetadata?.contentType || 'application/octet-stream',
          'Content-Disposition': `attachment; filename="${key.split('/').pop()}"`,
          'Cache-Control': 'public, max-age=86400',
          ...CORS,
        },
      });
    }

    // Latest download redirect (platform-aware)
    if (request.method === 'GET' && url.pathname === '/latest') {
      const platform = url.searchParams.get('platform') || 'mac-arm';
      const platformKey = PLATFORM_MAP[platform] || 'darwin-aarch64';
      try {
        const { results } = await env.DB.prepare(
          'SELECT r2_key FROM release_files WHERE version = (SELECT MAX(version) FROM release_files WHERE platform = ?) AND platform = ?'
        ).bind(platformKey, platformKey).all<{ r2_key: string }>();
        const file = results?.[0];
        if (file?.r2_key) {
          return Response.redirect(`https://dl-postgrestui.voltrus.id/releases/${encodeURIComponent(file.r2_key)}`, 302);
        }
      } catch (e) { console.error('Latest error:', e); }
      return new Response('No release found', { status: 404, headers: CORS });
    }

    // Downloads JSON endpoint
    if (request.method === 'GET' && url.pathname === '/downloads') {
      try {
        const versionRow = await env.DB.prepare('SELECT MAX(version) as v FROM release_files').first<{ v: number }>();
        if (!versionRow?.v) return Response.json({ error: 'No releases' }, { status: 404, headers: CORS });
        const { results } = await env.DB.prepare(
          'SELECT platform, filename, r2_key, size FROM release_files WHERE version = ?'
        ).bind(versionRow.v).all<{ platform: string; filename: string; r2_key: string; size: number }>();
        const base = 'https://dl-postgrestui.voltrus.id/releases/';
        const files: Record<string, { url: string; filename: string; size: number }> = {};
        for (const f of results ?? []) files[f.platform] = { url: `${base}${encodeURIComponent(f.r2_key)}`, filename: f.filename, size: f.size };
        return Response.json({ version: versionRow.v, files }, { headers: { ...CORS, 'Cache-Control': 'public, max-age=300' } });
      } catch (e) { return Response.json({ error: 'Error' }, { status: 500, headers: CORS }); }
    }

    // Health check
    if (url.pathname === '/' || url.pathname === '/health') {
      return new Response('OK - PostgresTUI downloads', { headers: { 'Content-Type': 'text/plain', ...CORS } });
    }

    return new Response('Not found', { status: 404, headers: CORS });
  },
};
