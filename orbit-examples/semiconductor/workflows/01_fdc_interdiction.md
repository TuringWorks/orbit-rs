# Semiconductor Workflow: FDC Interdiction

Fault Detection and Classification (FDC) analyzes real-time sensor data. If a tool drifts out of spec, it must be halted immediately to prevent scrapping millions of dollars of wafers.

## Flow Description

1. **Streaming Data (CQL)**
   - Tool `ETCH-01` streams RF Power and Pressure at 10Hz to `tool_traces`.

2. **Real-time Analysis (Middleware)**
   - Service polls/subscribes to stream.
   - Calculates statistical mean/std_dev.
   - Example: RF Power drops by 5% (Window Mean Shift).

3. **Interdiction (MES Interaction - SQL)**
   - FDC System issues "equipment hold" command.
   - Updates `tools` table status to `DOWN`.
   - Creating a hold on the current `lot`.

4. **Lineage Trace (AQL)**
   - Engineers query graph to find all other wafers processed by this tool in the last 2 hours for re-inspection.

## Step-by-Step Code Example

### 1. Interdiction Trigger (Pseudo-Code)

```python
# Analysis Logic
if current_rf_power < target_rf_power * 0.95:
    trigger_interdiction("ETCH-01", "RF Power Droop Detected")

def trigger_interdiction(tool_id, reason):
    # 1. Stop Tool (SQL)
    pg.execute(
        "UPDATE tools SET state='DOWN' WHERE tool_id=%s", 
        (tool_id,)
    )
    
    # 2. Hold Lot (SQL)
    active_lot = pg.query("SELECT lot_id FROM moves WHERE tool_id=%s AND time_out IS NULL", (tool_id,))
    pg.execute("UPDATE lots SET status='HOLD' WHERE lot_id=%s", (active_lot,))
    
    # 3. Log Event
    print(f"Tool {tool_id} INTERDICTED: {reason}")
```
