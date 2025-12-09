// ============================================================================
// OrbitRS Logistics Examples - Shipment Manifests (MongoDB)
// ============================================================================
// Storing Waybills and Customs documents
// ============================================================================

db = db.getSiblingDB('logistics_docs');

db.shipments.insertMany([
    {
        waybill: "WB-2024-888",
        origin: "Shanghai, CN",
        destination: "Los Angeles, US",
        carrier: "OceanNetwork",
        container_no: "MSCU1234567",
        items: [
            { desc: "Electronics", hs_code: "854231", weight_kg: 5000 },
            { desc: "Textiles", hs_code: "520811", weight_kg: 2000 }
        ],
        customs_status: "CLEARED",
        events: [
            { loc: "Port of Shanghai", status: "LOADED", ts: new Date("2024-05-01") },
            { loc: "Port of LA", status: "DISCHARGED", ts: new Date("2024-05-15") }
        ]
    }
]);
