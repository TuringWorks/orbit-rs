# Workflow: Add a Line (Postpaid)

This workflow describes the process of adding a new line to an existing customer's postpaid account.

## Overview

1.  **Eligibility Check** (SQL)
    - Check customer's credit class and maximum allowed lines.
    - Verify account status is 'Active' and no past due balance > threshold.

2.  **Number Allocation** (Redis)
    - Atomically pop a number from the available pool for the requested area code.
    - Reserve number temporarily to prevent race conditions.

3.  **SIM Provisioning** (MongoDB)
    - Validate ICCID (SIM card serial) status is 'Available'.
    - Link ICCID to the allocated MSISDN (phone number).

4.  **Activation** (Redis/CQL)
    - Update HLR/HSS cache in Redis for immediate network attachment.
    - Write permanent subscriber record to CQL.

5.  **Billing Update** (SQL)
    - Insert new subscription record into the billing system.
    - Prorate charges for the current cycle.

## Step-by-Step Execution

### Step 1: Eligibility (SQL)
```sql
SELECT credit_class, max_lines, current_lines, balance 
FROM customer_accounts 
WHERE account_id = 'ACC-998877';
-- Application Logic: If (current_lines < max_lines) AND (balance < 50.00) THEN Proceed
```

### Step 2: Reserve Number (Redis)
```bash
# Pop from available pool for Area Code 415
SPOP pool:msisdn:415
# Returns "14155550199"

# Reserve it
SET reservation:14155550199 "ACC-998877" EX 900
```

### Step 3: Provision SIM (MongoDB)
```javascript
db.sim_inventory.updateOne(
    { "iccid": "89014103211118510720", "status": "Available" },
    { $set: { "status": "Allocated", "msisdn": "14155550199", "account_id": "ACC-998877" } }
);
```

### Step 4: Network Activation (Redis)
```bash
# HLR Profile
HSET subscriber:14155550199 imsi "310410123456789" profile_id "LTE_UNL_DATA" status "Active"
```

### Step 5: Update Billing (SQL)
```sql
BEGIN;
INSERT INTO subscriptions (account_id, msisdn, plan_id, start_date, status)
VALUES ('ACC-998877', '14155550199', 'PLAN-UNL-5G', CURRENT_DATE, 'Active');

UPDATE customer_accounts 
SET current_lines = current_lines + 1 
WHERE account_id = 'ACC-998877';
COMMIT;
```
