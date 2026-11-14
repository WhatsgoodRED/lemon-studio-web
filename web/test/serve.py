# Static server for the web build (no special headers are needed: no SharedArrayBuffer).
import http.server, sys, functools
root = sys.argv[1]; port = int(sys.argv[2]) if len(sys.argv) > 2 else 8765
class H(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, '.wasm': 'application/wasm', '.js': 'text/javascript'}
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(('127.0.0.1', port), functools.partial(H, directory=root)).serve_forever()
