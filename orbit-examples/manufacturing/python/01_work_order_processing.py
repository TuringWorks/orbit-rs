#!/usr/bin/env python3
"""
OrbitRS Manufacturing Examples - Work Order Processing with ML
===============================================================
End-to-end work order processing with ML predictions
"""

import os
import sys
import psycopg2
import redis
import json
import uuid
from datetime import datetime, timedelta
from decimal import Decimal
import random

# Import shared configuration helpers from the common example utilities.
sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
)
from orbit_utils import require_env, env_int


class WorkOrderProcessingML:
    """Work order processing with ML-powered predictions"""
    
    def __init__(self):
        # Connect to OrbitRS PostgreSQL. Credentials come from environment
        # variables; never hardcode database passwords in source code.
        self.pg_conn = psycopg2.connect(
            host=os.getenv("ORBIT_PG_HOST", "localhost"),
            port=env_int("ORBIT_PG_PORT", 5432),
            database=os.getenv("ORBIT_PG_DB", "manufacturing"),
            user=os.getenv("ORBIT_PG_USER", "orbit"),
            password=require_env("ORBIT_PG_PASSWORD"),
        )
        
        # Connect to OrbitRS Redis
        self.redis_client = redis.Redis(
            host=os.getenv("ORBIT_REDIS_HOST", "localhost"),
            port=env_int("ORBIT_REDIS_PORT", 6379),
            db=0,
            decode_responses=True
        )
    
    def process_work_order(self, product_code, quantity, priority=5):
        """
        Complete work order processing with ML predictions
        
        Steps:
        1. ML: Demand forecast validation
        2. PostgreSQL: Create work order with BOM explosion
        3. ML: Production schedule optimization
        4. Redis: Add to queue with ML priority
        5. ML: Predict completion time and quality
        6. PostgreSQL: Reserve materials
        7. ML: Predictive maintenance check
        8. Redis: Update real-time status
        """
        
        print("=" * 80)
        print("WORK ORDER PROCESSING WITH ML")
        print("=" * 80)
        
        # Step 1: ML Demand Forecast
        print("\n[1/8] ML: Validating against demand forecast...")
        demand_forecast = self._get_demand_forecast(product_code)
        print(f"✓ Predicted demand (30d): {demand_forecast['predicted_demand']}")
        print(f"  Confidence interval: {demand_forecast['confidence_interval']}")
        print(f"  Trend: {demand_forecast['trend']}")
        
        # Step 2: Create work order
        print("\n[2/8] Creating work order in PostgreSQL...")
        wo_id, wo_number = self._create_work_order(product_code, quantity, priority)
        print(f"✓ Work order created: {wo_number}")
        
        # Step 3: ML Production Schedule
        print("\n[3/8] ML: Optimizing production schedule...")
        schedule = self._optimize_schedule(wo_id, product_code, quantity)
        print(f"✓ Assigned to line: {schedule['line_code']}")
        print(f"  Scheduled start: {schedule['start_time']}")
        print(f"  Expected completion: {schedule['completion_time']}")
        
        # Step 4: Add to queue with ML priority
        print("\n[4/8] Adding to production queue...")
        queue_position = self._add_to_queue(wo_id, schedule['ml_priority'])
        print(f"✓ Queue position: {queue_position}")
        
        # Step 5: ML Predictions
        print("\n[5/8] ML: Generating predictions...")
        predictions = self._generate_predictions(product_code, quantity, schedule['line_code'])
        print(f"✓ Predicted cycle time: {predictions['cycle_time']:.1f}s")
        print(f"  Predicted yield: {predictions['yield']:.1%}")
        print(f"  Quality confidence: {predictions['quality_confidence']:.1%}")
        
        # Step 6: Reserve materials
        print("\n[6/8] Reserving materials...")
        materials = self._reserve_materials(wo_id, product_code, quantity)
        print(f"✓ Reserved {len(materials)} component types")
        
        # Step 7: ML Predictive Maintenance
        print("\n[7/8] ML: Checking equipment health...")
        maintenance_check = self._check_equipment_health(schedule['line_code'])
        if maintenance_check['needs_maintenance']:
            print(f"⚠ Maintenance recommended: {maintenance_check['recommendation']}")
            print(f"  Predicted failure: {maintenance_check['predicted_failure_hours']}h")
        else:
            print("✓ Equipment health: GOOD")
        
        # Step 8: Update real-time status
        print("\n[8/8] Updating real-time status...")
        self._update_status(wo_id, wo_number, schedule, predictions)
        print("✓ Status updated in Redis")
        
        print("\n" + "=" * 80)
        print("WORK ORDER PROCESSING COMPLETE!")
        print("=" * 80)
        print(f"\nWork Order: {wo_number}")
        print(f"Product: {product_code}")
        print(f"Quantity: {quantity}")
        print(f"Line: {schedule['line_code']}")
        print(f"Expected Completion: {schedule['completion_time']}")
        print(f"Predicted Yield: {predictions['yield']:.1%}")
        
        return {
            'success': True,
            'work_order_id': wo_id,
            'work_order_number': wo_number,
            'schedule': schedule,
            'predictions': predictions
        }
    
    def _get_demand_forecast(self, product_code):
        """Get ML demand forecast from Redis"""
        forecast_key = f"ml:demand:{product_code}"
        forecast_data = self.redis_client.get(forecast_key)
        
        if forecast_data:
            return json.loads(forecast_data)
        
        # Simulate ML forecast
        return {
            'product_code': product_code,
            'predicted_demand': random.randint(100000, 150000),
            'confidence_interval': [95000, 155000],
            'trend': random.choice(['INCREASING', 'STABLE', 'DECREASING']),
            'seasonality_factor': round(random.uniform(0.9, 1.2), 2)
        }
    
    def _create_work_order(self, product_code, quantity, priority):
        """Create work order in PostgreSQL"""
        wo_id = str(uuid.uuid4())
        wo_number = f"WO-{datetime.now().strftime('%Y-%m%d')}-{wo_id[:8]}"
        
        with self.pg_conn.cursor() as cursor:
            # Get product and BOM
            cursor.execute("""
                SELECT p.product_id, bh.bom_id
                FROM products p
                JOIN bom_headers bh ON p.product_id = bh.product_id
                WHERE p.product_code = %s AND bh.status = 'ACTIVE'
                LIMIT 1
            """, (product_code,))
            
            row = cursor.fetchone()
            if not row:
                raise ValueError(f"Product {product_code} not found")
            
            product_id, bom_id = row
            
            # Create work order
            scheduled_start = datetime.now() + timedelta(hours=2)
            scheduled_end = scheduled_start + timedelta(hours=8)
            
            cursor.execute("""
                INSERT INTO work_orders (
                    work_order_id, work_order_number, product_id, bom_id,
                    quantity_ordered, priority, scheduled_start_date,
                    scheduled_end_date, status
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                wo_id, wo_number, product_id, bom_id, quantity,
                priority, scheduled_start, scheduled_end, 'PLANNED'
            ))
            
            # Create work order items from BOM
            cursor.execute("""
                INSERT INTO work_order_items (
                    work_order_item_id, work_order_id, component_id,
                    quantity_required, status
                )
                SELECT 
                    uuid_generate_v4(),
                    %s,
                    bi.component_id,
                    bi.quantity * %s,
                    'PENDING'
                FROM bom_items bi
                WHERE bi.bom_id = %s
            """, (wo_id, quantity, bom_id))
            
            self.pg_conn.commit()
        
        return wo_id, wo_number
    
    def _optimize_schedule(self, wo_id, product_code, quantity):
        """ML-optimized production scheduling"""
        # Simulate ML optimization
        lines = ['line-001', 'line-002', 'line-003']
        selected_line = random.choice(lines)
        
        start_time = datetime.now() + timedelta(hours=random.randint(2, 6))
        hours_needed = (quantity / 1000) * random.uniform(0.8, 1.2)
        completion_time = start_time + timedelta(hours=hours_needed)
        
        # ML priority score (0-1, higher = more urgent)
        ml_priority = round(random.uniform(0.5, 1.0), 2)
        
        # Store in Redis
        schedule_data = {
            'work_order_id': wo_id,
            'line_code': selected_line,
            'start_time': start_time.isoformat(),
            'completion_time': completion_time.isoformat(),
            'ml_priority': ml_priority,
            'utilization': round(random.uniform(0.85, 0.98), 2)
        }
        
        self.redis_client.setex(
            f"ml:schedule:{wo_id}",
            1800,
            json.dumps(schedule_data, default=str)
        )
        
        return schedule_data
    
    def _add_to_queue(self, wo_id, ml_priority):
        """Add work order to priority queue"""
        # Use ML priority score as sorted set score
        timestamp = datetime.now().timestamp()
        score = timestamp * (1 + ml_priority)  # Higher priority = higher score
        
        self.redis_client.zadd('queue:work-orders', {wo_id: score})
        
        # Get position
        rank = self.redis_client.zrevrank('queue:work-orders', wo_id)
        return rank + 1 if rank is not None else 1
    
    def _generate_predictions(self, product_code, quantity, line_code):
        """ML predictions for cycle time, yield, quality"""
        # Simulate ML predictions
        predictions = {
            'cycle_time': round(random.uniform(40, 50), 1),
            'yield': round(random.uniform(0.975, 0.995), 3),
            'quality_confidence': round(random.uniform(0.90, 0.98), 2),
            'predicted_defects': int(quantity * random.uniform(0.005, 0.025)),
            'model_versions': {
                'cycle_time': 'lstm_v2',
                'quality': 'random_forest_v3',
                'yield': 'xgboost_v2'
            }
        }
        
        return predictions
    
    def _reserve_materials(self, wo_id, product_code, quantity):
        """Reserve materials in Redis"""
        # Simplified - would query actual BOM in production
        components = [
            {'code': 'COMP-PCB-001', 'qty': quantity},
            {'code': 'COMP-DISPLAY-002', 'qty': quantity},
            {'code': 'COMP-BATTERY-003', 'qty': quantity}
        ]
        
        for comp in components:
            # Decrement available inventory
            self.redis_client.decrby(f"inventory:{comp['code']}", comp['qty'])
            
            # Set reservation
            self.redis_client.setex(
                f"inventory:reserved:{wo_id}:{comp['code']}",
                3600,
                comp['qty']
            )
        
        return components
    
    def _check_equipment_health(self, line_code):
        """ML predictive maintenance check"""
        health_key = f"ml:health:{line_code}"
        health_data = self.redis_client.get(health_key)
        
        if health_data:
            data = json.loads(health_data)
            return {
                'needs_maintenance': data['health_score'] < 0.85,
                'health_score': data['health_score'],
                'predicted_failure_hours': data.get('predicted_failure_hours', 0),
                'recommendation': data.get('recommendation', 'NONE')
            }
        
        # Simulate
        health_score = round(random.uniform(0.80, 0.95), 2)
        return {
            'needs_maintenance': health_score < 0.85,
            'health_score': health_score,
            'predicted_failure_hours': random.randint(100, 200),
            'recommendation': 'SCHEDULE_MAINTENANCE' if health_score < 0.85 else 'NONE'
        }
    
    def _update_status(self, wo_id, wo_number, schedule, predictions):
        """Update real-time status in Redis"""
        status_data = {
            'work_order_number': wo_number,
            'status': 'PLANNED',
            'line_code': schedule['line_code'],
            'scheduled_start': schedule['start_time'],
            'predicted_completion': schedule['completion_time'],
            'predicted_yield': predictions['yield'],
            'ml_priority': schedule['ml_priority'],
            'updated_at': datetime.now().isoformat()
        }
        
        self.redis_client.hmset(f"wo:status:{wo_number}", status_data)
        self.redis_client.expire(f"wo:status:{wo_number}", 86400)
    
    def close_connections(self):
        """Close database connections"""
        self.pg_conn.close()
        self.redis_client.close()


def main():
    """Example usage"""
    
    # Create workflow instance
    workflow = WorkOrderProcessingML()
    
    try:
        # Process work order with ML
        result = workflow.process_work_order(
            product_code='SMARTPHONE-X1',
            quantity=2000,
            priority=3
        )
        
        print("\n" + "=" * 80)
        print("WORKFLOW RESULT:")
        print(json.dumps(result, indent=2, default=str))
        
    finally:
        workflow.close_connections()


if __name__ == '__main__':
    main()
