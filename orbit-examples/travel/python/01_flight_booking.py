#!/usr/bin/env python3
"""
OrbitRS Travel Examples - Flight Booking Workflow
==================================================
End-to-end flight booking using PostgreSQL, Redis, MongoDB, and Neo4j
Demonstrates multi-protocol integration for travel booking platform
"""

import psycopg2
import redis
import pymongo
from neo4j import GraphDatabase
import json
import uuid
from datetime import datetime, timedelta, date
from decimal import Decimal

class FlightBookingWorkflow:
    """Complete flight booking workflow similar to Expedia/Orbitz"""
    
    def __init__(self):
        # Connect to OrbitRS PostgreSQL
        self.pg_conn = psycopg2.connect(
            host="localhost",
            port=5432,
            database="travel",
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
        self.mongo_db = self.mongo_client["travel"]
        
        # Connect to OrbitRS Neo4j
        self.neo4j_driver = GraphDatabase.driver(
            "bolt://localhost:7687",
            auth=("neo4j", "password")
        )
    
    def search_and_book_flight(self, customer_id, origin, destination, 
                                departure_date, return_date=None, 
                                passengers=1, cabin_class="ECONOMY"):
        """
        Complete flight search and booking workflow
        
        Steps:
        1. Check search cache (Redis)
        2. Search available flights (PostgreSQL)
        3. Get customer preferences (MongoDB)
        4. Calculate pricing with loyalty discount (Redis + PostgreSQL)
        5. Create booking (PostgreSQL)
        6. Update inventory (PostgreSQL + Redis)
        7. Award loyalty points (Redis)
        8. Store search preferences (MongoDB)
        9. Update recommendation graph (Neo4j)
        10. Send confirmation (Redis pub/sub)
        """
        
        print("=" * 80)
        print("FLIGHT BOOKING WORKFLOW - EXPEDIA/ORBITZ STYLE")
        print("=" * 80)
        
        # Step 1: Check search cache
        print("\n[1/10] Checking search cache...")
        cached_results = self._check_search_cache(origin, destination, departure_date)
        if cached_results:
            print(f"✓ Found {len(cached_results)} cached results")
        else:
            print("✓ No cache, will search database")
        
        # Step 2: Search available flights
        print("\n[2/10] Searching available flights...")
        flights = self._search_flights(origin, destination, departure_date, cabin_class)
        print(f"✓ Found {len(flights)} available flights")
        for i, flight in enumerate(flights[:3], 1):
            print(f"  {i}. {flight['flight_number']} - {flight['departure_time']} to {flight['arrival_time']} - ${flight['price']:.2f}")
        
        # Cache search results
        self._cache_search_results(origin, destination, departure_date, flights)
        
        # Step 3: Get customer preferences
        print("\n[3/10] Retrieving customer preferences...")
        customer_info = self._get_customer_info(customer_id)
        preferences = self._get_customer_preferences(customer_id)
        print(f"✓ Customer: {customer_info['name']}")
        print(f"  Loyalty Tier: {customer_info['loyalty_tier']}")
        print(f"  Points Balance: {customer_info['points']:,}")
        if preferences:
            print(f"  Preferred Airline: {preferences.get('preferred_airline', 'None')}")
            print(f"  Seat Preference: {preferences.get('seat_preference', 'None')}")
        
        # Select best flight (first one for demo)
        selected_flight = flights[0]
        print(f"\n✓ Selected: {selected_flight['flight_number']} - ${selected_flight['price']:.2f}")
        
        # Step 4: Calculate pricing with discounts
        print("\n[4/10] Calculating final pricing...")
        pricing = self._calculate_pricing(
            selected_flight, 
            passengers, 
            customer_info['loyalty_tier']
        )
        print(f"✓ Base Fare: ${pricing['base_fare']:.2f}")
        print(f"  Taxes & Fees: ${pricing['taxes_fees']:.2f}")
        if pricing['discount'] > 0:
            print(f"  Loyalty Discount ({customer_info['loyalty_tier']}): -${pricing['discount']:.2f}")
        print(f"  Total: ${pricing['total']:.2f}")
        print(f"  Points to Earn: {pricing['points_earned']:,}")
        
        # Step 5: Check seat availability
        print("\n[5/10] Checking seat availability...")
        availability = self._check_availability(
            selected_flight['flight_id'],
            departure_date,
            cabin_class,
            passengers
        )
        if not availability['available']:
            print(f"✗ Not enough seats available")
            return {'success': False, 'reason': 'NO_AVAILABILITY'}
        print(f"✓ {availability['seats_available']} seats available")
        
        # Step 6: Create booking
        print("\n[6/10] Creating flight booking...")
        booking_id = self._create_booking(
            customer_id,
            selected_flight,
            departure_date,
            passengers,
            cabin_class,
            pricing
        )
        booking_ref = f"BK{datetime.now().strftime('%Y%m%d')}{booking_id[:6].upper()}"
        print(f"✓ Booking created: {booking_ref}")
        
        # Step 7: Update inventory
        print("\n[7/10] Updating seat inventory...")
        self._update_inventory(
            selected_flight['flight_id'],
            departure_date,
            cabin_class,
            passengers
        )
        print(f"✓ Reserved {passengers} seat(s)")
        
        # Step 8: Award loyalty points
        print("\n[8/10] Awarding loyalty points...")
        new_balance = self._award_loyalty_points(
            customer_id,
            pricing['points_earned'],
            booking_id
        )
        print(f"✓ Earned {pricing['points_earned']:,} points")
        print(f"  New balance: {new_balance:,} points")
        
        # Step 9: Store preferences
        print("\n[9/10] Updating customer preferences...")
        self._update_preferences(
            customer_id,
            selected_flight['airline_code'],
            preferences.get('seat_preference', 'WINDOW') if preferences else 'WINDOW'
        )
        print("✓ Preferences updated")
        
        # Step 10: Update recommendation graph
        print("\n[10/10] Updating recommendation graph...")
        self._update_recommendations(
            customer_id,
            origin,
            destination,
            selected_flight['airline_code']
        )
        print("✓ Recommendations updated")
        
        # Send confirmation
        print("\n[FINAL] Sending booking confirmation...")
        self._send_confirmation(customer_id, booking_ref, selected_flight, pricing)
        print("✓ Confirmation sent!")
        
        print("\n" + "=" * 80)
        print("FLIGHT BOOKING COMPLETE!")
        print("=" * 80)
        print(f"\nBooking Reference: {booking_ref}")
        print(f"Flight: {selected_flight['flight_number']}")
        print(f"Route: {origin} → {destination}")
        print(f"Date: {departure_date}")
        print(f"Passengers: {passengers}")
        print(f"Total: ${pricing['total']:.2f}")
        print(f"Status: CONFIRMED")
        
        return {
            'success': True,
            'booking_id': booking_id,
            'booking_reference': booking_ref,
            'flight_number': selected_flight['flight_number'],
            'total_price': pricing['total'],
            'points_earned': pricing['points_earned']
        }
    
    def _check_search_cache(self, origin, destination, departure_date):
        """Check Redis cache for search results"""
        cache_key = f"search:flight:{origin}:{destination}:{departure_date}"
        cached = self.redis_client.get(cache_key)
        
        if cached:
            data = json.loads(cached)
            return data.get('results', [])
        return None
    
    def _search_flights(self, origin, destination, departure_date, cabin_class):
        """Search available flights in PostgreSQL"""
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                SELECT 
                    f.flight_id,
                    f.flight_number,
                    a.airline_code,
                    a.name AS airline_name,
                    orig.airport_code AS origin,
                    dest.airport_code AS destination,
                    f.departure_time,
                    f.arrival_time,
                    f.flight_duration_minutes,
                    fi.economy_available,
                    fi.business_class_available,
                    fi.first_class_available,
                    fp.total_price
                FROM flights f
                JOIN airlines a ON f.airline_id = a.airline_id
                JOIN airports orig ON f.origin_airport_id = orig.airport_id
                JOIN airports dest ON f.destination_airport_id = dest.airport_id
                JOIN flight_inventory fi ON f.flight_id = fi.flight_id
                JOIN flight_pricing fp ON f.flight_id = fp.flight_id 
                    AND fi.flight_date = fp.flight_date
                WHERE orig.airport_code = %s
                    AND dest.airport_code = %s
                    AND fi.flight_date = %s
                    AND fp.fare_class = %s
                    AND fi.status = 'SCHEDULED'
                    AND f.is_active = TRUE
                ORDER BY f.departure_time
                LIMIT 10
            """, (origin, destination, departure_date, cabin_class))
            
            flights = []
            for row in cursor.fetchall():
                flights.append({
                    'flight_id': str(row[0]),
                    'flight_number': row[1],
                    'airline_code': row[2],
                    'airline_name': row[3],
                    'origin': row[4],
                    'destination': row[5],
                    'departure_time': str(row[6]),
                    'arrival_time': str(row[7]),
                    'duration_minutes': row[8],
                    'seats_available': row[9] if cabin_class == 'ECONOMY' else row[10],
                    'price': float(row[12])
                })
            
            return flights
    
    def _cache_search_results(self, origin, destination, departure_date, flights):
        """Cache search results in Redis for 30 minutes"""
        cache_key = f"search:flight:{origin}:{destination}:{departure_date}"
        cache_data = {
            'results': flights,
            'timestamp': datetime.now().isoformat()
        }
        self.redis_client.setex(cache_key, 1800, json.dumps(cache_data, default=str))
    
    def _get_customer_info(self, customer_id):
        """Get customer information from PostgreSQL"""
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                SELECT first_name, last_name, loyalty_tier
                FROM customers WHERE customer_id = %s
            """, (customer_id,))
            row = cursor.fetchone()
            
            if not row:
                raise ValueError("Customer not found")
            
            # Get loyalty points from Redis
            points_key = f"loyalty:balance:{customer_id}"
            points_data = self.redis_client.hgetall(points_key)
            
            return {
                'name': f"{row[0]} {row[1]}",
                'loyalty_tier': row[2] or 'BRONZE',
                'points': int(points_data.get('points', 0)) if points_data else 0
            }
    
    def _get_customer_preferences(self, customer_id):
        """Get customer preferences from MongoDB"""
        prefs = self.mongo_db.customer_preferences.find_one(
            {'customer_id': customer_id}
        )
        return prefs
    
    def _calculate_pricing(self, flight, passengers, loyalty_tier):
        """Calculate final pricing with loyalty discounts"""
        base_fare = Decimal(str(flight['price'])) * passengers
        taxes_fees = base_fare * Decimal('0.15')  # 15% taxes
        
        # Loyalty discount
        discount_rates = {
            'BRONZE': 0,
            'SILVER': 0.05,
            'GOLD': 0.10,
            'PLATINUM': 0.15,
            'DIAMOND': 0.20
        }
        discount_rate = Decimal(str(discount_rates.get(loyalty_tier, 0)))
        discount = base_fare * discount_rate
        
        total = base_fare + taxes_fees - discount
        
        # Calculate points earned (1 point per dollar, with tier bonus)
        points_multiplier = {
            'BRONZE': 1.0,
            'SILVER': 1.25,
            'GOLD': 1.5,
            'PLATINUM': 2.0,
            'DIAMOND': 2.5
        }
        points_earned = int(float(total) * points_multiplier.get(loyalty_tier, 1.0))
        
        return {
            'base_fare': float(base_fare),
            'taxes_fees': float(taxes_fees),
            'discount': float(discount),
            'total': float(total),
            'points_earned': points_earned
        }
    
    def _check_availability(self, flight_id, flight_date, cabin_class, passengers):
        """Check seat availability in PostgreSQL and Redis"""
        # Check PostgreSQL
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                SELECT economy_available, business_class_available, first_class_available
                FROM flight_inventory
                WHERE flight_id = %s AND flight_date = %s
            """, (flight_id, flight_date))
            row = cursor.fetchone()
            
            if not row:
                return {'available': False, 'seats_available': 0}
            
            seats_map = {
                'ECONOMY': row[0],
                'BUSINESS': row[1],
                'FIRST': row[2]
            }
            seats_available = seats_map.get(cabin_class, 0)
            
            return {
                'available': seats_available >= passengers,
                'seats_available': seats_available
            }
    
    def _create_booking(self, customer_id, flight, flight_date, passengers, cabin_class, pricing):
        """Create booking in PostgreSQL"""
        booking_id = str(uuid.uuid4())
        booking_ref = f"BK{datetime.now().strftime('%Y%m%d')}{booking_id[:6].upper()}"
        
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO flight_bookings (
                    booking_id, booking_reference, flight_id, flight_date,
                    customer_id, num_passengers, fare_class, fare_type,
                    base_price, taxes_fees, total_price,
                    payment_status, booking_status, loyalty_points_earned
                ) VALUES (
                    %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                )
            """, (
                booking_id, booking_ref, flight['flight_id'], flight_date,
                customer_id, passengers, cabin_class, 'STANDARD',
                pricing['base_fare'], pricing['taxes_fees'], pricing['total'],
                'PAID', 'CONFIRMED', pricing['points_earned']
            ))
            
            self.pg_conn.commit()
        
        return booking_id
    
    def _update_inventory(self, flight_id, flight_date, cabin_class, passengers):
        """Update seat inventory in PostgreSQL and Redis"""
        column_map = {
            'ECONOMY': 'economy_available',
            'BUSINESS': 'business_class_available',
            'FIRST': 'first_class_available'
        }
        column = column_map.get(cabin_class)
        
        with self.pg_conn.cursor() as cursor:
            cursor.execute(f"""
                UPDATE flight_inventory
                SET {column} = {column} - %s,
                    total_booked = total_booked + %s
                WHERE flight_id = %s AND flight_date = %s
            """, (passengers, passengers, flight_id, flight_date))
            
            self.pg_conn.commit()
        
        # Update Redis cache
        # (In production, invalidate search cache here)
    
    def _award_loyalty_points(self, customer_id, points, booking_id):
        """Award loyalty points in Redis"""
        points_key = f"loyalty:balance:{customer_id}"
        self.redis_client.hincrby(points_key, 'points', points)
        self.redis_client.hincrby(points_key, 'lifetime_points', points)
        
        # Record transaction
        activity_key = f"activity:points:{customer_id}"
        activity = {
            'type': 'EARN',
            'amount': points,
            'booking': booking_id,
            'date': datetime.now().isoformat()
        }
        self.redis_client.lpush(activity_key, json.dumps(activity))
        self.redis_client.ltrim(activity_key, 0, 49)  # Keep last 50
        
        new_balance = int(self.redis_client.hget(points_key, 'points'))
        return new_balance
    
    def _update_preferences(self, customer_id, airline_code, seat_preference):
        """Update customer preferences in MongoDB"""
        self.mongo_db.customer_preferences.update_one(
            {'customer_id': customer_id},
            {
                '$set': {
                    'preferred_airline': airline_code,
                    'seat_preference': seat_preference,
                    'last_booking_date': datetime.now()
                },
                '$inc': {'total_bookings': 1}
            },
            upsert=True
        )
    
    def _update_recommendations(self, customer_id, origin, destination, airline):
        """Update Neo4j recommendation graph"""
        with self.neo4j_driver.session() as session:
            # Create customer node if not exists
            session.run("""
                MERGE (c:Customer {customer_id: $customer_id})
                SET c.last_booking = datetime()
            """, customer_id=customer_id)
            
            # Create route relationship
            session.run("""
                MERGE (r:Route {route: $route})
                WITH r
                MATCH (c:Customer {customer_id: $customer_id})
                MERGE (c)-[b:BOOKED]->(r)
                ON CREATE SET b.count = 1, b.first_booking = datetime()
                ON MATCH SET b.count = b.count + 1, b.last_booking = datetime()
            """, route=f"{origin}:{destination}", customer_id=customer_id)
    
    def _send_confirmation(self, customer_id, booking_ref, flight, pricing):
        """Send confirmation via Redis pub/sub"""
        notification = {
            'customer_id': customer_id,
            'booking_reference': booking_ref,
            'flight_number': flight['flight_number'],
            'total': pricing['total'],
            'message': f'Your flight {flight["flight_number"]} is confirmed! Booking: {booking_ref}',
            'timestamp': datetime.now().isoformat()
        }
        
        self.redis_client.publish('notifications:bookings', json.dumps(notification))
    
    def close_connections(self):
        """Close all database connections"""
        self.pg_conn.close()
        self.redis_client.close()
        self.mongo_client.close()
        self.neo4j_driver.close()


def main():
    """Example usage"""
    
    # Sample flight booking
    customer_id = str(uuid.uuid4())  # In production, use actual customer ID
    
    # Create workflow instance
    workflow = FlightBookingWorkflow()
    
    try:
        # Book a flight from SFO to JFK
        result = workflow.search_and_book_flight(
            customer_id=customer_id,
            origin='SFO',
            destination='JFK',
            departure_date=date.today() + timedelta(days=30),
            passengers=1,
            cabin_class='ECONOMY'
        )
        
        print("\n" + "=" * 80)
        print("WORKFLOW RESULT:")
        print(json.dumps(result, indent=2, default=str))
        
    except Exception as e:
        print(f"\n✗ Error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        workflow.close_connections()


if __name__ == '__main__':
    main()
