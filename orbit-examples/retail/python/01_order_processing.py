#!/usr/bin/env python3
"""
OrbitRS Retail Examples - Order Processing Workflow
====================================================
End-to-end order processing using PostgreSQL, Redis, MongoDB, and Neo4j
"""

import psycopg2
import redis
import pymongo
from neo4j import GraphDatabase
import json
import uuid
from datetime import datetime, timedelta, date
from decimal import Decimal

class RetailOrderProcessing:
    """Complete order processing workflow across all OrbitRS protocols"""
    
    def __init__(self):
        # Connect to OrbitRS PostgreSQL
        self.pg_conn = psycopg2.connect(
            host="localhost",
            port=5432,
            database="retail",
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
        self.mongo_db = self.mongo_client["retail"]
        
        # Connect to OrbitRS Neo4j
        self.neo4j_driver = GraphDatabase.driver(
            "bolt://localhost:7687",
            auth=("neo4j", "password")
        )
    
    def process_order(self, cart_data, customer_data, payment_data, shipping_data):
        """
        Complete order processing workflow
        
        Steps:
        1. Redis: Validate cart and check inventory
        2. PostgreSQL: Create order and reserve inventory
        3. Redis: Apply dynamic pricing and promotions
        4. PostgreSQL: Process payment
        5. MongoDB: Store order receipt
        6. Neo4j: Update purchase graph for recommendations
        7. Redis: Clear cart and update analytics
        """
        
        print("=" * 80)
        print("RETAIL ORDER PROCESSING WORKFLOW")
        print("=" * 80)
        
        # Step 1: Validate cart
        print("\n[1/8] Validating shopping cart...")
        cart_items = self._validate_cart(cart_data['cart_id'])
        print(f"✓ Cart validated: {len(cart_items)} items")
        
        # Step 2: Check inventory availability
        print("\n[2/8] Checking inventory availability...")
        inventory_check = self._check_inventory(cart_items)
        if not inventory_check['available']:
            print(f"✗ Insufficient inventory for: {inventory_check['unavailable_items']}")
            return {'success': False, 'reason': 'INSUFFICIENT_INVENTORY'}
        print("✓ All items in stock")
        
        # Step 3: Calculate pricing with promotions
        print("\n[3/8] Calculating order total with promotions...")
        pricing = self._calculate_pricing(cart_items, customer_data.get('coupon_code'))
        print(f"✓ Subtotal: ${pricing['subtotal']:.2f}")
        print(f"  Discount: -${pricing['discount']:.2f}")
        print(f"  Tax: ${pricing['tax']:.2f}")
        print(f"  Shipping: ${pricing['shipping']:.2f}")
        print(f"  Total: ${pricing['total']:.2f}")
        
        # Step 4: Create order in PostgreSQL
        print("\n[4/8] Creating order in PostgreSQL...")
        order_id = self._create_order(cart_items, customer_data, pricing, shipping_data)
        order_number = f"ORD-{datetime.now().strftime('%Y%m%d')}-{order_id[:8]}"
        print(f"✓ Order created: {order_number}")
        
        # Step 5: Reserve inventory
        print("\n[5/8] Reserving inventory...")
        self._reserve_inventory(order_id, cart_items)
        print("✓ Inventory reserved")
        
        # Step 6: Process payment
        print("\n[6/8] Processing payment...")
        payment_result = self._process_payment(order_id, pricing['total'], payment_data)
        if not payment_result['success']:
            print(f"✗ Payment failed: {payment_result['reason']}")
            self._cancel_order(order_id)
            return {'success': False, 'reason': 'PAYMENT_FAILED'}
        print(f"✓ Payment processed: {payment_result['transaction_id']}")
        
        # Step 7: Store order receipt in MongoDB
        print("\n[7/8] Storing order receipt in MongoDB...")
        receipt_id = self._store_receipt(order_id, order_number, cart_items, pricing, customer_data)
        print(f"✓ Receipt stored: {receipt_id}")
        
        # Step 8: Update recommendation graph
        print("\n[8/8] Updating recommendation graph in Neo4j...")
        self._update_recommendation_graph(customer_data['customer_id'], cart_items)
        print("✓ Recommendation graph updated")
        
        # Finalize: Clear cart and update analytics
        print("\n[FINAL] Finalizing order...")
        self._finalize_order(cart_data['cart_id'], order_id, pricing)
        print("✓ Order finalized!")
        
        print("\n" + "=" * 80)
        print("ORDER PROCESSING COMPLETE!")
        print("=" * 80)
        print(f"\nOrder Number: {order_number}")
        print(f"Total Amount: ${pricing['total']:.2f}")
        print(f"Items: {len(cart_items)}")
        print(f"Status: CONFIRMED")
        
        return {
            'success': True,
            'order_id': order_id,
            'order_number': order_number,
            'total': pricing['total'],
            'receipt_id': receipt_id
        }
    
    def _validate_cart(self, cart_id):
        """Validate cart from Redis"""
        cart_key = f"cart:{cart_id}"
        cart_items = self.redis_client.hgetall(cart_key)
        
        if not cart_items:
            raise ValueError("Cart is empty or expired")
        
        items = []
        for key, quantity in cart_items.items():
            if key.startswith('product:'):
                sku = key.replace('product:', '')
                items.append({'sku': sku, 'quantity': int(quantity)})
        
        return items
    
    def _check_inventory(self, cart_items):
        """Check inventory availability in Redis"""
        unavailable = []
        
        for item in cart_items:
            stock = self.redis_client.get(f"inventory:{item['sku']}")
            if stock is None or int(stock) < item['quantity']:
                unavailable.append(item['sku'])
        
        return {
            'available': len(unavailable) == 0,
            'unavailable_items': unavailable
        }
    
    def _calculate_pricing(self, cart_items, coupon_code=None):
        """Calculate pricing with dynamic pricing and promotions"""
        subtotal = Decimal('0')
        
        # Get prices from Redis cache or PostgreSQL
        for item in cart_items:
            price_key = f"price:dynamic:{item['sku']}"
            price_data = self.redis_client.get(price_key)
            
            if price_data:
                price_info = json.loads(price_data)
                unit_price = Decimal(str(price_info['current_price']))
            else:
                # Fallback to base price (would query PostgreSQL in production)
                unit_price = Decimal('99.99')  # Simplified
            
            item['unit_price'] = unit_price
            item['total'] = unit_price * item['quantity']
            subtotal += item['total']
        
        # Apply coupon discount
        discount = Decimal('0')
        if coupon_code:
            coupon_data = self.redis_client.hgetall(f"coupon:{coupon_code}")
            if coupon_data and coupon_data.get('discount_pct'):
                discount_pct = Decimal(coupon_data['discount_pct']) / 100
                discount = subtotal * discount_pct
                # Increment coupon usage
                self.redis_client.hincrby(f"coupon:{coupon_code}", 'uses', 1)
        
        # Calculate tax and shipping
        tax = (subtotal - discount) * Decimal('0.0875')  # 8.75% tax
        shipping = Decimal('9.99') if subtotal < 100 else Decimal('0')  # Free shipping over $100
        
        total = subtotal - discount + tax + shipping
        
        return {
            'subtotal': float(subtotal),
            'discount': float(discount),
            'tax': float(tax),
            'shipping': float(shipping),
            'total': float(total)
        }
    
    def _create_order(self, cart_items, customer_data, pricing, shipping_data):
        """Create order in PostgreSQL"""
        order_id = str(uuid.uuid4())
        order_number = f"ORD-{datetime.now().strftime('%Y%m%d')}-{order_id[:8]}"
        
        with self.pg_conn.cursor() as cursor:
            # Create order
            cursor.execute("""
                INSERT INTO orders (
                    order_id, order_number, customer_id, email,
                    order_type, channel, subtotal, shipping_amount,
                    tax_amount, discount_amount, total_amount,
                    payment_method, payment_status, fulfillment_type,
                    fulfillment_status, status,
                    shipping_first_name, shipping_last_name,
                    shipping_address_1, shipping_city, shipping_state,
                    shipping_postal_code, shipping_phone
                ) VALUES (
                    %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s,
                    %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                )
            """, (
                order_id, order_number, customer_data['customer_id'],
                customer_data['email'], 'WEB', 'ONLINE',
                pricing['subtotal'], pricing['shipping'], pricing['tax'],
                pricing['discount'], pricing['total'],
                'CREDIT_CARD', 'PENDING', 'SHIP_TO_HOME', 'PENDING', 'PENDING',
                shipping_data['first_name'], shipping_data['last_name'],
                shipping_data['address'], shipping_data['city'],
                shipping_data['state'], shipping_data['zip'], shipping_data['phone']
            ))
            
            # Create order items
            for item in cart_items:
                cursor.execute("""
                    INSERT INTO order_items (
                        order_item_id, order_id, sku, product_name,
                        quantity, unit_price, total_amount, fulfillment_status
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                """, (
                    str(uuid.uuid4()), order_id, item['sku'],
                    f"Product {item['sku']}", item['quantity'],
                    item['unit_price'], item['total'], 'PENDING'
                ))
            
            self.pg_conn.commit()
        
        return order_id
    
    def _reserve_inventory(self, order_id, cart_items):
        """Reserve inventory in Redis"""
        for item in cart_items:
            # Decrement available inventory
            self.redis_client.decrby(f"inventory:{item['sku']}", item['quantity'])
            
            # Set reservation
            self.redis_client.setex(
                f"inventory:reserved:order-{order_id}:{item['sku']}",
                900,  # 15 minutes
                item['quantity']
            )
    
    def _process_payment(self, order_id, amount, payment_data):
        """Process payment (simplified)"""
        # In production, would integrate with payment gateway
        transaction_id = f"TXN-{uuid.uuid4()}"
        
        # Update order payment status
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                UPDATE orders
                SET payment_status = 'PAID',
                    transaction_id = %s,
                    status = 'CONFIRMED',
                    confirmed_at = CURRENT_TIMESTAMP
                WHERE order_id = %s
            """, (transaction_id, order_id))
            self.pg_conn.commit()
        
        return {'success': True, 'transaction_id': transaction_id}
    
    def _store_receipt(self, order_id, order_number, cart_items, pricing, customer_data):
        """Store order receipt in MongoDB"""
        receipt = {
            'order_id': order_id,
            'order_number': order_number,
            'customer': {
                'customer_id': customer_data['customer_id'],
                'email': customer_data['email'],
                'name': customer_data.get('name', 'Customer')
            },
            'items': [
                {
                    'sku': item['sku'],
                    'quantity': item['quantity'],
                    'unit_price': float(item['unit_price']),
                    'total': float(item['total'])
                }
                for item in cart_items
            ],
            'pricing': pricing,
            'order_date': datetime.now(),
            'receipt_url': f"s3://receipts/{order_number}.pdf",
            'created_at': datetime.now()
        }
        
        result = self.mongo_db.order_receipts.insert_one(receipt)
        return str(result.inserted_id)
    
    def _update_recommendation_graph(self, customer_id, cart_items):
        """Update Neo4j recommendation graph"""
        with self.neo4j_driver.session() as session:
            # Create customer node if not exists
            session.run("""
                MERGE (c:Customer {customer_id: $customer_id})
                SET c.last_purchase = datetime()
            """, customer_id=customer_id)
            
            # Create product nodes and purchase relationships
            for item in cart_items:
                session.run("""
                    MERGE (p:Product {sku: $sku})
                    WITH p
                    MATCH (c:Customer {customer_id: $customer_id})
                    MERGE (c)-[r:PURCHASED]->(p)
                    ON CREATE SET r.count = 1, r.first_purchase = datetime()
                    ON MATCH SET r.count = r.count + 1, r.last_purchase = datetime()
                """, sku=item['sku'], customer_id=customer_id)
            
            # Create frequently bought together relationships
            if len(cart_items) > 1:
                for i, item1 in enumerate(cart_items):
                    for item2 in cart_items[i+1:]:
                        session.run("""
                            MATCH (p1:Product {sku: $sku1})
                            MATCH (p2:Product {sku: $sku2})
                            MERGE (p1)-[r:BOUGHT_WITH]-(p2)
                            ON CREATE SET r.count = 1
                            ON MATCH SET r.count = r.count + 1
                        """, sku1=item1['sku'], sku2=item2['sku'])
    
    def _cancel_order(self, order_id):
        """Cancel order and release inventory"""
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                UPDATE orders
                SET status = 'CANCELLED',
                    cancelled_at = CURRENT_TIMESTAMP
                WHERE order_id = %s
            """, (order_id,))
            self.pg_conn.commit()
    
    def _finalize_order(self, cart_id, order_id, pricing):
        """Clear cart and update analytics"""
        # Clear cart
        self.redis_client.delete(f"cart:{cart_id}")
        self.redis_client.delete(f"cart:meta:{cart_id}")
        
        # Update analytics
        today = date.today().isoformat()
        self.redis_client.incr(f"analytics:sales:count:{today}")
        self.redis_client.incrbyfloat(f"analytics:sales:revenue:{today}", pricing['total'])
        
        # Mark cart as converted
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                UPDATE shopping_carts
                SET status = 'CONVERTED',
                    converted_to_order_id = %s
                WHERE cart_id = %s
            """, (order_id, cart_id))
            self.pg_conn.commit()
    
    def close_connections(self):
        """Close all database connections"""
        self.pg_conn.close()
        self.redis_client.close()
        self.mongo_client.close()
        self.neo4j_driver.close()


def main():
    """Example usage"""
    
    # Sample cart data
    cart_data = {
        'cart_id': 'sess-abc123'
    }
    
    # Sample customer data
    customer_data = {
        'customer_id': 'cust-001',
        'email': 'customer@example.com',
        'name': 'John Doe',
        'coupon_code': 'WINTER30'
    }
    
    # Sample payment data
    payment_data = {
        'method': 'CREDIT_CARD',
        'card_last_four': '1234'
    }
    
    # Sample shipping data
    shipping_data = {
        'first_name': 'John',
        'last_name': 'Doe',
        'address': '123 Main St',
        'city': 'San Francisco',
        'state': 'CA',
        'zip': '94102',
        'phone': '+14155551234'
    }
    
    # Create workflow instance
    workflow = RetailOrderProcessing()
    
    try:
        # Execute order processing workflow
        result = workflow.process_order(
            cart_data, customer_data, payment_data, shipping_data
        )
        
        print("\n" + "=" * 80)
        print("WORKFLOW RESULT:")
        print(json.dumps(result, indent=2, default=str))
        
    finally:
        workflow.close_connections()


if __name__ == '__main__':
    main()
