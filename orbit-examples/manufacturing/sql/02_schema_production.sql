-- ============================================================================
-- OrbitRS Manufacturing Examples - Production & Work Orders Schema
-- ============================================================================
-- Work orders, assembly lines, production runs
-- ============================================================================
-- ============================================================================
-- ASSEMBLY LINES
-- ============================================================================
CREATE TABLE assembly_lines (
    line_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    line_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Line Type
    line_type VARCHAR(30) CHECK (
        line_type IN (
            'FINAL_ASSEMBLY',
            'SUB_ASSEMBLY',
            'TESTING',
            'PACKAGING'
        )
    ),
    -- Capacity
    target_units_per_hour INTEGER,
    max_capacity_per_hour INTEGER,
    -- Status
    status VARCHAR(20) DEFAULT 'IDLE' CHECK (
        status IN (
            'IDLE',
            'RUNNING',
            'CHANGEOVER',
            'MAINTENANCE',
            'DOWN'
        )
    ),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_assembly_lines_status ON assembly_lines(status);
-- ============================================================================
-- ASSEMBLY STATIONS
-- ============================================================================
CREATE TABLE assembly_stations (
    station_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    line_id UUID NOT NULL REFERENCES assembly_lines(line_id),
    station_code VARCHAR(50) UNIQUE NOT NULL,
    -- Station Details
    name VARCHAR(200) NOT NULL,
    sequence_number INTEGER NOT NULL,
    -- Station Type
    station_type VARCHAR(30) CHECK (
        station_type IN (
            'MANUAL',
            'SEMI_AUTO',
            'AUTOMATED',
            'INSPECTION',
            'TEST'
        )
    ),
    -- Cycle Time
    standard_cycle_time_seconds INTEGER,
    -- Status
    status VARCHAR(20) DEFAULT 'IDLE',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_assembly_stations_line ON assembly_stations(line_id);
-- ============================================================================
-- WORK ORDERS
-- ============================================================================
CREATE TABLE work_orders (
    work_order_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    work_order_number VARCHAR(50) UNIQUE NOT NULL,
    -- Product
    product_id UUID NOT NULL REFERENCES products(product_id),
    variant_id UUID REFERENCES product_variants(variant_id),
    bom_id UUID NOT NULL REFERENCES bom_headers(bom_id),
    -- Quantity
    quantity_ordered INTEGER NOT NULL CHECK (quantity_ordered > 0),
    quantity_completed INTEGER DEFAULT 0,
    quantity_scrapped INTEGER DEFAULT 0,
    -- Assembly Line
    line_id UUID REFERENCES assembly_lines(line_id),
    -- Priority
    priority INTEGER DEFAULT 5,
    -- 1 = highest, 10 = lowest
    -- Dates
    scheduled_start_date TIMESTAMP,
    scheduled_end_date TIMESTAMP,
    actual_start_date TIMESTAMP,
    actual_end_date TIMESTAMP,
    -- Status
    status VARCHAR(20) DEFAULT 'PLANNED' CHECK (
        status IN (
            'PLANNED',
            'RELEASED',
            'IN_PROGRESS',
            'COMPLETED',
            'CANCELLED',
            'ON_HOLD'
        )
    ),
    -- Notes
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_work_orders_product ON work_orders(product_id);
CREATE INDEX idx_work_orders_line ON work_orders(line_id);
CREATE INDEX idx_work_orders_status ON work_orders(status);
CREATE INDEX idx_work_orders_scheduled_start ON work_orders(scheduled_start_date);
-- ============================================================================
-- WORK ORDER ITEMS (Material Requirements)
-- ============================================================================
CREATE TABLE work_order_items (
    work_order_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    work_order_id UUID NOT NULL REFERENCES work_orders(work_order_id) ON DELETE CASCADE,
    component_id UUID NOT NULL REFERENCES components(component_id),
    -- Quantity
    quantity_required DECIMAL(12, 4) NOT NULL,
    quantity_issued DECIMAL(12, 4) DEFAULT 0,
    quantity_consumed DECIMAL(12, 4) DEFAULT 0,
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'ISSUED',
            'CONSUMED',
            'RETURNED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_work_order_items_wo ON work_order_items(work_order_id);
CREATE INDEX idx_work_order_items_component ON work_order_items(component_id);
-- ============================================================================
-- PRODUCTION RUNS
-- ============================================================================
CREATE TABLE production_runs (
    run_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    run_number VARCHAR(50) UNIQUE NOT NULL,
    work_order_id UUID NOT NULL REFERENCES work_orders(work_order_id),
    line_id UUID NOT NULL REFERENCES assembly_lines(line_id),
    -- Quantity
    quantity_planned INTEGER NOT NULL,
    quantity_produced INTEGER DEFAULT 0,
    quantity_passed INTEGER DEFAULT 0,
    quantity_failed INTEGER DEFAULT 0,
    -- Timing
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    -- Performance
    target_cycle_time_seconds INTEGER,
    actual_cycle_time_seconds INTEGER,
    -- Status
    status VARCHAR(20) DEFAULT 'PLANNED' CHECK (
        status IN (
            'PLANNED',
            'RUNNING',
            'PAUSED',
            'COMPLETED',
            'ABORTED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_production_runs_wo ON production_runs(work_order_id);
CREATE INDEX idx_production_runs_line ON production_runs(line_id);
CREATE INDEX idx_production_runs_status ON production_runs(status);
-- ============================================================================
-- STATION COMPLETIONS (Unit-level tracking)
-- ============================================================================
CREATE TABLE station_completions (
    completion_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    run_id UUID NOT NULL REFERENCES production_runs(run_id),
    station_id UUID NOT NULL REFERENCES assembly_stations(station_id),
    -- Unit Tracking
    serial_number VARCHAR(100),
    unit_number INTEGER,
    -- Timing
    started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    cycle_time_seconds INTEGER,
    -- Operator
    operator_id UUID,
    -- References staff table
    -- Status
    status VARCHAR(20) DEFAULT 'IN_PROGRESS' CHECK (
        status IN (
            'IN_PROGRESS',
            'COMPLETED',
            'FAILED',
            'REWORK'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_station_completions_run ON station_completions(run_id);
CREATE INDEX idx_station_completions_station ON station_completions(station_id);
CREATE INDEX idx_station_completions_serial ON station_completions(serial_number);
-- ============================================================================
-- MACHINE DOWNTIME
-- ============================================================================
CREATE TABLE machine_downtime (
    downtime_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    line_id UUID REFERENCES assembly_lines(line_id),
    station_id UUID REFERENCES assembly_stations(station_id),
    -- Downtime Details
    downtime_type VARCHAR(30) CHECK (
        downtime_type IN (
            'PLANNED_MAINTENANCE',
            'UNPLANNED_MAINTENANCE',
            'CHANGEOVER',
            'BREAKDOWN',
            'MATERIAL_SHORTAGE',
            'QUALITY_HOLD'
        )
    ),
    -- Timing
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP,
    duration_minutes INTEGER,
    -- Description
    reason TEXT,
    resolution TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_machine_downtime_line ON machine_downtime(line_id);
CREATE INDEX idx_machine_downtime_station ON machine_downtime(station_id);
CREATE INDEX idx_machine_downtime_started ON machine_downtime(started_at);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_assembly_lines_updated_at BEFORE
UPDATE ON assembly_lines FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_work_orders_updated_at BEFORE
UPDATE ON work_orders FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active work orders with progress
CREATE VIEW v_active_work_orders AS
SELECT wo.work_order_id,
    wo.work_order_number,
    p.product_code,
    p.name AS product_name,
    wo.quantity_ordered,
    wo.quantity_completed,
    wo.quantity_scrapped,
    ROUND(
        (
            wo.quantity_completed::DECIMAL / wo.quantity_ordered
        ) * 100,
        2
    ) AS completion_pct,
    wo.status,
    al.line_code,
    wo.scheduled_start_date,
    wo.scheduled_end_date
FROM work_orders wo
    JOIN products p ON wo.product_id = p.product_id
    LEFT JOIN assembly_lines al ON wo.line_id = al.line_id
WHERE wo.status IN ('RELEASED', 'IN_PROGRESS');
-- Production line efficiency
CREATE VIEW v_line_efficiency AS
SELECT al.line_id,
    al.line_code,
    al.name,
    COUNT(DISTINCT pr.run_id) AS total_runs,
    SUM(pr.quantity_produced) AS total_produced,
    AVG(pr.actual_cycle_time_seconds) AS avg_cycle_time,
    al.target_units_per_hour,
    ROUND(
        (
            SUM(pr.quantity_produced)::DECIMAL / NULLIF(
                SUM(
                    EXTRACT(
                        EPOCH
                        FROM (pr.completed_at - pr.started_at)
                    ) / 3600
                ),
                0
            )
        ),
        2
    ) AS actual_units_per_hour
FROM assembly_lines al
    LEFT JOIN production_runs pr ON al.line_id = pr.line_id
    AND pr.status = 'COMPLETED'
    AND pr.completed_at >= CURRENT_DATE - INTERVAL '7 days'
GROUP BY al.line_id,
    al.line_code,
    al.name,
    al.target_units_per_hour;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE work_orders IS 'Production work orders';
COMMENT ON TABLE production_runs IS 'Actual production runs on assembly lines';
COMMENT ON TABLE assembly_lines IS 'Manufacturing assembly lines';
COMMENT ON TABLE assembly_stations IS 'Stations within assembly lines';