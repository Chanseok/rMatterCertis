import { invoke } from "@tauri-apps/api/core";
import { For, createSignal, onMount } from "solid-js";

interface AnalyticsMappingDiagnostics {
    product_details_with_ids: number;
    product_details_total: number;
    bridge_rows: number;
    distinct_bridge_products: number;
    device_types_total: number;
    device_types_with_type_id: number;
    device_types_type_id_null: number;
    analytics_rows_total: number;
    analytics_with_device_type: number;
    analytics_distinct_products_total: number;
    analytics_distinct_products_mapped: number;
    sample_unmapped: any[];
    product_details_json_valid_ids: number;
    json_each_expanded_rows: number;
    sample_primary_device_type_ids: any[];
    sample_expanded_values: any[];
    distinct_json_values_count: number;
    unmatched_json_values_count: number;
    sample_unmatched_json_values: any[];
    sample_non_numeric_json_values: any[];
    sample_device_type_ids: number[];
    mapping_coverage_pct: number;
}

function DiagnosticItem(props: { title: string; value: any; status?: "ok" | "warn" | "error" | "info" }) {
    const statusClass = {
        ok: "text-green-500",
        warn: "text-yellow-500",
        error: "text-red-500",
        info: "text-blue-400",
    }[props.status || "info"];

    return (
        <div class="p-2 border-b border-gray-700">
            <span class="font-semibold">{props.title}: </span>
            <span class={`font-mono ${statusClass}`}>{typeof props.value === 'object' ? JSON.stringify(props.value, null, 2) : props.value.toString()}</span>
        </div>
    );
}

export function DatabaseDiagnostics() {
    const [report, setReport] = createSignal<AnalyticsMappingDiagnostics | null>(null);
    const [error, setError] = createSignal<string | null>(null);
    const [isLoading, setIsLoading] = createSignal(true);

    onMount(async () => {
        try {
            setIsLoading(true);
            const result = await invoke<AnalyticsMappingDiagnostics>("diagnostics_analytics_mapping");
            setReport(result);
        } catch (e: any) {
            setError(e.toString());
        } finally {
            setIsLoading(false);
        }
    });

    const renderReport = (r: AnalyticsMappingDiagnostics) => (
        <div class="bg-gray-800 text-white p-4 rounded-lg shadow-lg font-sans">
            <h2 class="text-xl font-bold mb-4 border-b-2 border-blue-500 pb-2">Database Mapping Diagnostics</h2>
            
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div class="bg-gray-900 p-3 rounded">
                    <h3 class="font-bold text-lg mb-2 text-cyan-400">Analytics View</h3>
                    <DiagnosticItem title="Total Rows" value={r.analytics_rows_total} />
                    <DiagnosticItem title="Mapped Rows" value={r.analytics_with_device_type} status={r.analytics_with_device_type > 0 ? "ok" : "error"} />
                    <DiagnosticItem title="Mapping Success Ratio" value={`${r.mapping_coverage_pct.toFixed(2)}%`} status={r.mapping_coverage_pct > 80 ? "ok" : (r.mapping_coverage_pct > 10 ? "warn" : "error")} />
                </div>

                <div class="bg-gray-900 p-3 rounded">
                    <h3 class="font-bold text-lg mb-2 text-cyan-400">Product Details Table</h3>
                    <DiagnosticItem title="Total Rows" value={r.product_details_total} />
                    <DiagnosticItem title="Rows with primary_device_type_ids" value={r.product_details_with_ids} status={r.product_details_with_ids > 0 ? "ok" : "error"} />
                    <DiagnosticItem title="Rows with VALID JSON ids" value={r.product_details_json_valid_ids} status={r.product_details_json_valid_ids === r.product_details_with_ids ? "ok" : "warn"} />
                </div>

                <div class="bg-gray-900 p-3 rounded">
                    <h3 class="font-bold text-lg mb-2 text-cyan-400">Device Types Table</h3>
                    <DiagnosticItem title="Total Rows" value={r.device_types_total} />
                    <DiagnosticItem title="Rows with NULL type_id" value={r.device_types_type_id_null} status={r.device_types_type_id_null === 0 ? "ok" : "error"} />
                </div>

                <div class="bg-gray-900 p-3 rounded">
                    <h3 class="font-bold text-lg mb-2 text-cyan-400">Join Mismatches</h3>
                    <DiagnosticItem title="Unmatched JSON Values Count" value={r.unmatched_json_values_count} status={r.unmatched_json_values_count === 0 ? "ok" : (r.unmatched_json_values_count > 0 ? "warn" : "info")} />
                    <DiagnosticItem title="Bridge Table Rows" value={r.bridge_rows} status={r.bridge_rows > 0 ? "ok" : "error"} />
                </div>
            </div>

            <div class="mt-4 bg-gray-900 p-3 rounded">
                <h3 class="font-bold text-lg mb-2 text-yellow-400">Samples & Details</h3>
                <h4 class="font-semibold mt-2">Sample `primary_device_type_ids` (from product_details):</h4>
                <pre class="bg-black p-2 rounded mt-1 text-sm overflow-x-auto">
                    <For each={r.sample_primary_device_type_ids}>{(item) => <div>{JSON.stringify(item)}</div>}</For>
                </pre>

                <h4 class="font-semibold mt-2">Sample Device Type IDs:</h4>
                <pre class="bg-black p-2 rounded mt-1 text-sm overflow-x-auto">
                    <For each={r.sample_device_type_ids}>{(item) => <div>{item}</div>}</For>
                </pre>

                <h4 class="font-semibold mt-2">Sample Unmatched JSON Values:</h4>
                <pre class="bg-black p-2 rounded mt-1 text-sm overflow-x-auto">
                    <For each={r.sample_unmatched_json_values}>{(item) => <div>{JSON.stringify(item)}</div>}</For>
                </pre>

                <h4 class="font-semibold mt-2">Sample Unmapped Products:</h4>
                <pre class="bg-black p-2 rounded mt-1 text-sm overflow-x-auto">
                    <For each={r.sample_unmapped}>{(item) => <div>{JSON.stringify(item)}</div>}</For>
                </pre>
            </div>
        </div>
    );

    return (
        <div>
            {isLoading() && <p>Loading diagnostics...</p>}
            {error() && <p class="text-red-500">Error: {error()}</p>}
            {report() && renderReport(report()!)}
        </div>
    );
}
