const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = 3001;
const STATIC_DIR = path.join(__dirname, 'static');

const server = http.createServer((req, res) => {
    // Enable CORS for Tauri
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (req.method === 'OPTIONS') {
        res.statusCode = 204;
        res.end();
        return;
    }

    // New Build Notification Endpoint
    if (req.method === 'POST' && req.url === '/notify-build') {
        let body = '';
        req.on('data', chunk => { body += chunk.toString(); });
        req.on('end', () => {
            try {
                const data = JSON.parse(body);
                console.log(`[BUILD] 🚀 New build completed: v${data.version}`);
                console.log(`[BUILD] 📝 Notes: ${data.notes}`);
                res.statusCode = 200;
                res.end('OK');
            } catch (e) {
                console.error('[BUILD] ❌ Received invalid build notification');
                res.statusCode = 400;
                res.end('Invalid JSON');
            }
        });
        return;
    }

    let urlPath = req.url === '/' ? '/latest.json' : req.url;
    let filePath = path.join(STATIC_DIR, urlPath);
    
    // Safety check: ensure file is within STATIC_DIR
    const relative = path.relative(STATIC_DIR, filePath);
    if (relative.startsWith('..') || path.isAbsolute(relative)) {
        res.statusCode = 403;
        res.end('Forbidden');
        return;
    }

    fs.readFile(filePath, (err, data) => {
        if (err) {
            if (err.code === 'ENOENT') {
                console.log(`[HTTP] 404 - Not Found: ${req.url}`);
                res.statusCode = 404;
                res.end('File not found');
            } else {
                console.error(`[HTTP] 500 - Error reading ${req.url}: ${err.message}`);
                res.statusCode = 500;
                res.end('Internal server error');
            }
            return;
        }

        console.log(`[HTTP] 200 - Serving: ${req.url} (${data.length} bytes)`);
        const ext = path.extname(filePath);
        let contentType = 'application/octet-stream';
        if (ext === '.json') contentType = 'application/json';
        if (ext === '.exe') contentType = 'application/vnd.microsoft.portable-executable';
        
        res.setHeader('Content-Type', contentType);
        res.end(data);
    });
});

server.listen(PORT, '0.0.0.0', () => {
    console.log(`Update server running at http://0.0.0.0:${PORT}`);
    console.log(`Serving files from: ${STATIC_DIR}`);
});
