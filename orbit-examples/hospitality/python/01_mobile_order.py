#!/usr/bin/env python3
"""
OrbitRS Hospitality Examples - Mobile Order Processing
=======================================================
End-to-end mobile order workflow using PostgreSQL, Redis, MongoDB, and Neo4j
"""

import psycopg2
import redis
import pymongo
from neo4j import GraphDatabase
import json
import uuid
from datetime import datetime, timedelta
from decimal import Decimal

class MobileOrderProcessing:
    """Complete mobile order processing workflow for coffeehouse/restaurant"""
    
    def __init__(self):
        # Connect to OrbitRS PostgreSQL
        self.pg_conn = psycopg2.connect(
            host="localhost",
            port=5432,
            database="hospitality",
            user="orbit",
            password="orbit"
        )
        
        # Connect to OrbitRS Redis
        self.redis_client = redis.Redis(
            host="localhost",
            port=6379,
            db=0,
            decode_responses=True
        )
        
        # Connect to OrbitRS MongoDB
        self.mongo_client = pymongo.MongoClient("mongodb://localhost:27017/")
        self.mongo_db = self.mongo_client["hospitality"]
        
        # Connect to OrbitRS Neo4j
        self.neo4j_driver = GraphDatabase.driver(
            "bolt://localhost:7687",
            auth=("neo4j", "password")
        )
    
    def process_mobile_order(self, customer_id, store_id, items, pickup_time=None):
        """
        Complete mobile order processing workflow
        
        Steps:
        1. Redis: Check customer session and loyalty status
        2. PostgreSQL: Validate menu items and calculate pricing
        3. Redis: Check inventory availability
        4. PostgreSQL: Create order
        5. Redis: Add to order queue and update loyalty points
        6. MongoDB: Store customer preferences
        7. Neo4j: Update recommendation graph
        8. Redis: Send notification
        """
        
        print("=" * 80)
        print("MOBILE ORDER PROCESSING WORKFLOW")
        print("=" * 80)
        
        # Step 1: Get customer info from cache
        print("\n[1/8] Retrieving customer information...")
        customer_info = self._get_customer_info(customer_id)
        print(f"✓ Customer: {customer_info['name']}")
        print(f"  Loyalty Tier: {customer_info['loyalty_tier']}")
        print(f"  Points: {customer_info['points']}")
        
        # Step 2: Validate items and calculate pricing
        print("\n[2/8] Validating menu items and calculating total...")
        order_details = self._calculate_order(items, customer_info)
        print(f"✓ Subtotal: ${order_details['subtotal']:.2f}")
        print(f"  Tax: ${order_details['tax']:.2f}")
        print(f"  Total: ${order_details['total']:.2f}")
        print(f"  Points to earn: {order_details['points_earned']}")
        
        # Step 3: Check inventory
        print("\n[3/8] Checking ingredient availability...")
        inventory_check = self._check_inventory(items)
        if not inventory_check['available']:
            print(f"✗ Unavailable items: {inventory_check['unavailable']}")
            return {'success': False, 'reason': 'ITEMS_UNAVAILABLE'}
        print("✓ All items available")
        
        # Step 4: Create order in PostgreSQL
        print("\n[4/8] Creating order in PostgreSQL...")
        order_id = self._create_order(customer_id, store_id, items, order_details, pickup_time)
        order_number = f"{datetime.now().strftime('%Y%m%d')}-{order_id[:8]}"
        print(f"✓ Order created: {order_number}")
        
        # Step 5: Add to order queue
        print("\n[5/8] Adding to order queue...")
        queue_position = self._add_to_queue(order_id, store_id, pickup_time)
        wait_time = self._estimate_wait_time(store_id)
        print(f"✓ Queue position: {queue_position}")
        print(f"  Estimated wait: {wait_time} minutes")
        
        # Step 6: Update loyalty points
        print("\n[6/8] Updating loyalty points...")
        new_balance = self._update_loyalty_points(customer_id, order_details['points_earned'])
        print(f"✓ Points earned: {order_details['points_earned']}")
        print(f"  New balance: {new_balance}")
        
        # Step 7: Store preferences in MongoDB
        print("\n[7/8] Storing order preferences...")
        self._store_preferences(customer_id, items)
        print("✓ Preferences updated")
        
        # Step 8: Update recommendation graph
        print("\n[8/8] Updating recommendation graph...")
        self._update_recommendations(customer_id, items)
        print("✓ Recommendations updated")
        
        # Send notification
        print("\n[FINAL] Sending order confirmation...")
        self._send_notification(customer_id, order_number, wait_time)
        print("✓ Notification sent!")
        
        print("\n" + "=" * 80)
        print("MOBILE ORDER COMPLETE!")
        print("=" * 80)
        print(f"\nOrder Number: {order_number}")
        print(f"Pickup Time: {pickup_time or f'~{wait_time} minutes'}")
        print(f"Total: ${order_details['total']:.2f}")
        print(f"Status: CONFIRMED")
        
        return {
            'success': True,
            'order_id': order_id,
            'order_number': order_number,
            'wait_time_minutes': wait_time,
            'total': order_details['total']
        }
    
    def _get_customer_info(self, customer_id):
        """Get customer info from Redis cache"""
        cache_key = f"cache:customer:{customer_id}"
        cached = self.redis_client.get(cache_key)
        
        if cached:
            return json.loads(cached)
        
        # Fallback to PostgreSQL
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                SELECT first_name, last_name, loyalty_tier
                FROM customers WHERE customer_id = %s
            """, (customer_id,))
            row = cursor.fetchone()
            
            if not row:
                raise ValueError("Customer not found")
            
            # Get loyalty points from Redis
            loyalty_data = self.redis_client.hgetall(f"loyalty:balance:{customer_id}")
            
            customer_info = {
                'name': f"{row[0]} {row[1]}",
                'loyalty_tier': row[2] or 'BRONZE',
                'points': int(loyalty_data.get('points', 0))
            }
            
            # Cache for 30 minutes
            self.redis_client.setex(cache_key, 1800, json.dumps(customer_info))
            
            return customer_info
    
    def _calculate_order(self, items, customer_info):
        """Calculate order total with loyalty discounts"""
        subtotal = Decimal('0')
        points_earned = 0
        
        for item in items:
            # Simplified pricing (would query PostgreSQL in production)
            item_price = Decimal('5.95')  # Base price
            
            # Add modifier costs
            for modifier in item.get('modifiers', []):
                if modifier in ['Extra Shot', 'Oat Milk']:
                    item_price += Decimal('0.75')
            
            item['price'] = float(item_price)
            subtotal += item_price
        
        # Calculate tax
        tax = subtotal * Decimal('0.0875')  # 8.75%
        total = subtotal + tax
        
        # Calculate loyalty points (1 point per dollar)
        points_earned = int(total)
        
        # Loyalty tier bonus
        if customer_info['loyalty_tier'] == 'GOLD':
            points_earned = int(points_earned * 1.5)
        elif customer_info['loyalty_tier'] == 'PLATINUM':
            points_earned = int(points_earned * 2)
        
        return {
            'subtotal': float(subtotal),
            'tax': float(tax),
            'total': float(total),
            'points_earned': points_earned
        }
    
    def _check_inventory(self, items):
        """Check ingredient availability in Redis"""
        unavailable = []
        
        for item in items:
            # Check if item is out of stock
            if self.redis_client.sismember('inventory:out-of-stock', item['name']):
                unavailable.append(item['name'])
        
        return {
            'available': len(unavailable) == 0,
            'unavailable': unavailable
        }
    
    def _create_order(self, customer_id, store_id, items, order_details, pickup_time):
        """Create order in PostgreSQL"""
        order_id = str(uuid.uuid4())
        order_number = f"{datetime.now().strftime('%Y%m%d')}-{order_id[:8]}"
        
        # Calculate promised time
        if pickup_time:
            promised_at = pickup_time
        else:
            promised_at = datetime.now() + timedelta(minutes=10)
        
        with self.pg_conn.cursor() as cursor:
            # Create order
            cursor.execute("""
                INSERT INTO orders (
                    order_id, order_number, store_id, customer_id,
                    order_type, order_source, subtotal, tax_amount,
                    total_amount, payment_status, fulfillment_status,
                    promised_at, loyalty_points_earned
                ) VALUES (
                    %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                )
            """, (
                order_id, order_number, store_id, customer_id,
                'MOBILE', 'MOBILE_APP', order_details['subtotal'],
                order_details['tax'], order_details['total'],
                'PAID', 'PENDING', promised_at, order_details['points_earned']
            ))
            
            # Create order items (simplified)
            for item in items:
                cursor.execute("""
                    INSERT INTO order_items (
                        order_item_id, order_id, item_name,
                        quantity, unit_price, total_price, prep_status
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s)
                """, (
                    str(uuid.uuid4()), order_id, item['name'],
                    item.get('quantity', 1), item['price'], item['price'],
                    'PENDING'
                ))
            
            self.pg_conn.commit()
        
        return order_id
    
    def _add_to_queue(self, order_id, store_id, pickup_time):
        """Add order to Redis queue"""
        if pickup_time:
            # Scheduled order (sorted set)
            timestamp = int(pickup_time.timestamp())
            self.redis_client.zadd('queue:orders:mobile', {order_id: timestamp})
        else:
            # Immediate order (list)
            self.redis_client.lpush('queue:orders:in-store', order_id)
        
        # Get queue position
        queue_length = self.redis_client.llen('queue:orders:in-store')
        return queue_length
    
    def _estimate_wait_time(self, store_id):
        """Estimate wait time from Redis"""
        wait_data = self.redis_client.get(f"wait:time:{store_id}")
        
        if wait_data:
            data = json.loads(wait_data)
            return data.get('current_wait_minutes', 10)
        
        return 10  # Default 10 minutes
    
    def _update_loyalty_points(self, customer_id, points_earned):
        """Update loyalty points in Redis"""
        self.redis_client.hincrby(f"loyalty:balance:{customer_id}", 'points', points_earned)
        self.redis_client.hincrby(f"loyalty:balance:{customer_id}", 'lifetime_points', points_earned)
        
        # Get new balance
        new_balance = int(self.redis_client.hget(f"loyalty:balance:{customer_id}", 'points'))
        
        return new_balance
    
    def _store_preferences(self, customer_id, items):
        """Store customer preferences in MongoDB"""
        preferences = {
            'customer_id': customer_id,
            'favorite_items': [item['name'] for item in items],
            'last_order_date': datetime.now(),
            'updated_at': datetime.now()
        }
        
        self.mongo_db.customer_preferences.update_one(
            {'customer_id': customer_id},
            {'$set': preferences},
            upsert=True
        )
    
    def _update_recommendations(self, customer_id, items):
        """Update Neo4j recommendation graph"""
        with self.neo4j_driver.session() as session:
            # Create customer node if not exists
            session.run("""
                MERGE (c:Customer {customer_id: $customer_id})
                SET c.last_order = datetime()
            """, customer_id=customer_id)
            
            # Create ordered relationships
            for item in items:
                session.run("""
                    MERGE (i:MenuItem {name: $item_name})
                    WITH i
                    MATCH (c:Customer {customer_id: $customer_id})
                    MERGE (c)-[r:ORDERED]->(i)
                    ON CREATE SET r.count = 1, r.first_order = datetime()
                    ON MATCH SET r.count = r.count + 1, r.last_order = datetime()
                """, item_name=item['name'], customer_id=customer_id)
    
    def _send_notification(self, customer_id, order_number, wait_time):
        """Send notification via Redis pub/sub"""
        notification = {
            'customer_id': customer_id,
            'order_number': order_number,
            'message': f'Your order #{order_number} is confirmed! Ready in ~{wait_time} minutes.',
            'timestamp': datetime.now().isoformat()
        }
        
        self.redis_client.publish('notifications:orders', json.dumps(notification))
    
    def close_connections(self):
        """Close all database connections"""
        self.pg_conn.close()
        self.redis_client.close()
        self.mongo_client.close()
        self.neo4j_driver.close()


def main():
    """Example usage"""
    
    # Sample mobile order
    customer_id = 'cust-001'
    store_id = 'store-sf-001'
    
    items = [
        {
            'name': 'Coconut Latte',
            'modifiers': ['Oat Milk', 'Extra Shot'],
            'quantity': 1
        },
        {
            'name': 'Blueberry Muffin',
            'quantity': 1
        }
    ]
    
    # Create workflow instance
    workflow = MobileOrderProcessing()
    
    try:
        # Process mobile order
        result = workflow.process_mobile_order(
            customer_id=customer_id,
            store_id=store_id,
            items=items,
            pickup_time=None  # ASAP
        )
        
        print("\n" + "=" * 80)
        print("WORKFLOW RESULT:")
        print(json.dumps(result, indent=2, default=str))
        
    finally:
        workflow.close_connections()


if __name__ == '__main__':
    main()
