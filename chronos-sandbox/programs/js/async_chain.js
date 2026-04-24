// JavaScript async/await chain for async function testing.
//
// STATUS: Placeholder - may have runtime availability issues
// (Requires Node.js installed and node in PATH)
//
// KNOWN_BEHAVIOR:
// - Function calls: main -> async level1 -> async level2 -> async level3
// - Async function chain with await at each level
// - Each level does some async work (simulated with setTimeout/promises)
// - Clear async call chain showing async function boundaries
//
// EXPECTED EVENTS:
// - Function entry: main, level1, level2, level3 (async function calls)
// - Promise creation, resolution events
// - Async operation boundaries (await points)
// - Event loop tick events
// - No exceptions (clean async flow)
//
// USEFUL FOR:
// - Testing async function call capture
// - Verifying async stack trace reconstruction
// - Testing promise resolution event capture
// - Validating event loop interaction tracking

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function level3(value) {
    await sleep(1);
    return value * 3;
}

async function level2(value) {
    const result = await level3(value);
    return result + 2;
}

async function level1(value) {
    const result = await level2(value);
    return result + 1;
}

async function main() {
    console.log("Starting async chain test...");
    const result = await level1(10);
    console.log(`Async chain complete. Result: ${result}`);
    console.log("Call chain: main -> level1 -> level2 -> level3 (all async)");
    console.log("(Each level awaits the next)");
}

main().catch(err => {
    console.error("Error in async chain:", err);
    process.exit(1);
});
