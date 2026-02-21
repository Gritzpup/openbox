const http = require('http');

const PORT = 3002;

const server = http.createServer((req, res) => {
    // Enable CORS
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (req.method === 'OPTIONS') {
        res.statusCode = 204;
        res.end();
        return;
    }

    if (req.method === 'POST' && req.url === '/log') {
        let body = '';
        req.on('data', chunk => { body += chunk.toString(); });
        req.on('end', () => {
            try {
                const data = JSON.parse(body);
                const timestamp = new Date().toLocaleTimeString();
                const level = data.level || 'INFO';
                const source = data.source || 'UNKNOWN';
                const message = data.message || '';
                
                console.log(`[${timestamp}] [${level}] [${source}] ${message}`);
                
                res.statusCode = 200;
                res.end('OK');
            } catch (e) {
                console.error('❌ Received invalid log format:', e.message);
                res.statusCode = 400;
                res.end('Invalid JSON');
            }
        });
        return;
    }

    res.statusCode = 404;
    res.end('Not Found');
});

server.listen(PORT, '0.0.0.0', () => {
    console.log(`🚀 OpenBox Log Server running at http://0.0.0.0:${PORT}`);
    console.log('📡 Listening for telemetry from all instances...');
});
