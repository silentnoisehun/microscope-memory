/**
 * Microscope Memory WASM - Minimal TypeScript usage example
 * 
 * Használat Vite/React projektekben:
 * 
 * 1. Másold a pkg/ könyvtár tartalmát a projekted public/ mappájába
 *    (pl. public/microscope-memory/)
 * 
 * 2. Telepítsd a modult (opcionális):
 *    Másold a pkg/ mappát a projektedbe, majd:
 *    npm install ./microscope-memory-wasm
 * 
 * 3. Használat:
 */

// --- Aszinkron inicializálás ---
import init, { MicroscopeWasm, WasmBlock } from './pkg/microscope_memory.js';

export class MicroscopeClient {
    private memory: MicroscopeWasm | null = null;

    async initialize() {
        await init();
        this.memory = new MicroscopeWasm();
        console.log(Microscope Memory ready. Blocks: );
    }

    /** Memória tárolása */
    store(text: string, layer: string = 'session', importance: number = 5) {
        if (!this.memory) throw new Error('Not initialized');
        this.memory.store(text, layer, importance);
    }

    /** Keresés természetes nyelven */
    recall(query: string, k: number = 10): WasmBlock[] {
        if (!this.memory) throw new Error('Not initialized');
        return this.memory.recall(query, k);
    }

    /** Bináris adatok betöltése (meta.bin + microscope.bin + data.bin) */
    loadBinary(meta: Uint8Array, headers: Uint8Array, data: Uint8Array) {
        if (!this.memory) throw new Error('Not initialized');
        this.memory.load_binary(meta, headers, data);
    }

    /** Applog betöltése (APv2 formátum) */
    loadAppend(data: Uint8Array) {
        if (!this.memory) throw new Error('Not initialized');
        this.memory.load_append(data);
    }

    /** Applog exportálása APv2 formátumban */
    exportAppend(): Uint8Array {
        if (!this.memory) throw new Error('Not initialized');
        return this.memory.export_append();
    }

    get blockCount(): number {
        return this.memory?.block_count() ?? 0;
    }

    get isLoaded(): boolean {
        return this.memory?.is_loaded() ?? false;
    }
}

// --- Példa használat ---
async function example() {
    const client = new MicroscopeClient();
    await client.initialize();

    // Tárolás
    client.store('Az első memória bejegyzésem', 'session', 5);
    client.store('Fontos projekt információ', 'long_term', 9);

    // Visszakeresés
    const results = client.recall('projekt');
    for (const r of results) {
        console.log([]  (dist: ));
    }

    // Export
    const blob = client.exportAppend();
    console.log(Exportált méret:  bytes);
}

// React hook példa:
// import { useState, useEffect } from 'react';
// 
// function useMicroscope() {
//   const [client] = useState(() => new MicroscopeClient());
//   const [ready, setReady] = useState(false);
// 
//   useEffect(() => {
//     client.initialize().then(() => setReady(true));
//   }, []);
// 
//   return { client, ready };
// }
