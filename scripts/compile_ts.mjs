// Compile a Vega-Lite spec with the upstream TypeScript implementation and
// report the time spent inside `compile()`.
import {compile} from 'vega-lite';

let input = '';
for await (const chunk of process.stdin) {
  input += chunk;
}

try {
  const spec = JSON.parse(input);
  const t0 = performance.now();
  const result = compile(spec);
  const ms = performance.now() - t0;
  process.stdout.write(JSON.stringify({ok: true, vega: result.spec, normalized: result.normalized, ms}));
} catch (err) {
  process.stdout.write(JSON.stringify({ok: false, error: String(err && err.stack ? err.stack : err)}));
  process.exitCode = 1;
}
