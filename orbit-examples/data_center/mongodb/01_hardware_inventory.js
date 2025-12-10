// ============================================================================
// OrbitRS Data Center Examples - Hardware Inventory (MongoDB)
// ============================================================================
// Tracking hardware assets across terrestrial and orbital data centers
// ============================================================================

db = db.getSiblingDB('datacenter_ops');

// Insert hardware inventory with nested specifications
db.hardware_inventory.insertMany([
    {
        asset_tag: "SRV-USW2-001",
        site: {
            name: "US-West-2",
            type: "Terrestrial",
            location: {
                city: "Portland",
                state: "OR",
                coordinates: { lat: 45.52, lon: -122.67 }
            }
        },
        rack: {
            rack_id: "RK-A01",
            u_position: 10,
            power_zone: "A"
        },
        hardware_type: "Server",
        manufacturer: "Dell",
        model: "PowerEdge R750",
        serial_number: "SN-2024-ABC123",
        state: "Active",
        specs: {
            cpu: {
                model: "Intel Xeon Gold 6330",
                cores: 56,
                threads: 112,
                clock_ghz: 2.0
            },
            memory_gb: 512,
            storage: [
                { type: "NVMe", capacity_tb: 3.2, count: 8 }
            ],
            network: {
                nics: 4,
                speed_gbps: 100
            }
        },
        provisioning: {
            os: "RHEL 9",
            cluster: "k8s-prod-west",
            workload: "AI Inference"
        },
        maintenance_window: "Sunday 02:00-06:00 UTC",
        warranty_expires: new Date("2027-03-15")
    },
    {
        asset_tag: "SAT-ORB-ALPHA-CM01",
        site: {
            name: "Orbital-Shell-Alpha",
            type: "Orbital_Shell",
            location: {
                orbit: "LEO",
                altitude_km: 550,
                inclination_deg: 53
            }
        },
        rack: {
            rack_id: "SAT-101",
            bay: "Compute-Bay-1"
        },
        hardware_type: "Compute_Module",
        manufacturer: "NVIDIA",
        model: "Jetson-Space-Hardened",
        serial_number: "SN-SPACE-001",
        state: "Active",
        specs: {
            cpu: {
                model: "ARM Cortex-A78AE",
                cores: 12,
                rad_hardened: true
            },
            gpu: {
                model: "Ampere",
                cuda_cores: 2048,
                tensor_cores: 64
            },
            memory_gb: 32,
            power_watts: 30
        },
        provisioning: {
            os: "Linux-Space-OS",
            mission: "Edge AI Processing",
            uplink_station: "GS-Fairbanks"
        },
        last_contact: new Date(),
        orbit_parameters: {
            apogee_km: 555,
            perigee_km: 545,
            period_min: 95.6
        }
    }
]);

// Insert maintenance events
db.maintenance_events.insertMany([
    {
        event_id: "MAINT-2024-001",
        asset_tag: "SRV-USW2-001",
        event_type: "Firmware Update",
        technician: "John Smith",
        scheduled_at: new Date("2024-03-17T03:00:00Z"),
        completed_at: new Date("2024-03-17T03:45:00Z"),
        status: "Completed",
        notes: "BIOS updated to v2.5.1, iDRAC firmware to v6.10"
    },
    {
        event_id: "MAINT-2024-002",
        asset_tag: "SAT-ORB-ALPHA-CM01",
        event_type: "Health Check",
        technician: "AI_Agent_Orbital",
        scheduled_at: new Date(),
        status: "Pending",
        notes: "Automated orbital health diagnostic"
    }
]);

// Query: Find all active terrestrial servers
db.hardware_inventory.find({
    "site.type": "Terrestrial",
    "hardware_type": "Server",
    "state": "Active"
});

// Query: Find orbital assets with low power consumption
db.hardware_inventory.find({
    "site.type": "Orbital_Shell",
    "specs.power_watts": { $lt: 50 }
});

// Aggregation: Hardware count and total memory by site
db.hardware_inventory.aggregate([
    {
        $group: {
            _id: "$site.name",
            asset_count: { $sum: 1 },
            total_memory_gb: { $sum: "$specs.memory_gb" },
            total_cpu_cores: { $sum: "$specs.cpu.cores" }
        }
    },
    { $sort: { total_cpu_cores: -1 } }
]);

// Aggregation: Assets with warranty expiring soon
db.hardware_inventory.aggregate([
    {
        $match: {
            warranty_expires: {
                $gte: new Date(),
                $lte: new Date(Date.now() + 90 * 24 * 60 * 60 * 1000)
            }
        }
    },
    {
        $project: {
            asset_tag: 1,
            "site.name": 1,
            manufacturer: 1,
            model: 1,
            warranty_expires: 1,
            days_until_expiry: {
                $divide: [
                    { $subtract: ["$warranty_expires", new Date()] },
                    1000 * 60 * 60 * 24
                ]
            }
        }
    }
]);
