#!/usr/bin/env python3
"""
OrbitRS Telco Examples - Subscriber Onboarding Workflow
========================================================
End-to-end subscriber onboarding using PostgreSQL, Redis, MongoDB, and Neo4j
"""

import os
import sys
import psycopg2
import redis
import pymongo
from neo4j import GraphDatabase
import json
import uuid
from datetime import datetime, timedelta, date
from decimal import Decimal

# Import shared configuration helpers from the common example utilities.
sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
)
from orbit_utils import require_env, env_int


class TelcoSubscriberOnboarding:
    """Complete subscriber onboarding workflow across all OrbitRS protocols"""

    def __init__(self):
        # Connect to OrbitRS PostgreSQL. Credentials come from environment
        # variables; never hardcode database passwords in source code.
        self.pg_conn = psycopg2.connect(
            host=os.getenv("ORBIT_PG_HOST", "localhost"),
            port=env_int("ORBIT_PG_PORT", 5432),
            database=os.getenv("ORBIT_PG_DB", "telco"),
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

        # Connect to OrbitRS MongoDB
        self.mongo_client = pymongo.MongoClient(os.getenv("ORBIT_MONGO_URL", "mongodb://localhost:27017/"))
        self.mongo_db = self.mongo_client["telco"]

        # Connect to OrbitRS Neo4j. Credentials come from environment variables.
        self.neo4j_driver = GraphDatabase.driver(
            os.getenv("ORBIT_NEO4J_URL", "bolt://localhost:7687"),
            auth=(os.getenv("ORBIT_NEO4J_USER", "neo4j"), require_env("ORBIT_NEO4J_PASSWORD")),
        )

    def onboard_subscriber(self, subscriber_data, plan_data, payment_method):
        """
        Complete subscriber onboarding workflow

        Steps:
        1. PostgreSQL: Create subscriber, account, address
        2. PostgreSQL: Assign plan and create billing cycle
        3. Redis: Cache subscriber profile and activate session
        4. MongoDB: Store contract documents
        5. Neo4j: Create subscriber node and relationships
        6. PostgreSQL: Record initial charges
        """

        print("=" * 80)
        print("TELCO SUBSCRIBER ONBOARDING WORKFLOW")
        print("=" * 80)

        # Step 1: Create subscriber
        print("\n[1/8] Creating subscriber in PostgreSQL...")
        subscriber_id = self._create_subscriber(subscriber_data)
        print(f"✓ Subscriber created: {subscriber_id}")
        print(f"  MSISDN: {subscriber_data['msisdn']}")

        # Step 2: Create account
        print("\n[2/8] Creating account in PostgreSQL...")
        account_id = self._create_account(subscriber_id, subscriber_data)
        print(f"✓ Account created: {account_id}")

        # Step 3: Add address
        print("\n[3/8] Adding service address...")
        address_id = self._add_address(subscriber_id, account_id, subscriber_data['address'])
        print(f"✓ Address added: {address_id}")

        # Step 4: Assign plan
        print("\n[4/8] Assigning service plan...")
        plan_assignment = self._assign_plan(subscriber_id, account_id, plan_data)
        print(f"✓ Plan assigned: {plan_data['plan_name']}")
        print(f"  Monthly charge: ${plan_data['monthly_charge']:.2f}")

        # Step 5: Add payment method
        print("\n[5/8] Adding payment method...")
        payment_method_id = self._add_payment_method(subscriber_id, account_id, payment_method)
        print(f"✓ Payment method added")

        # Step 6: Cache subscriber profile in Redis
        print("\n[6/8] Caching subscriber profile in Redis...")
        self._cache_subscriber_profile(subscriber_id, subscriber_data, plan_data)
        print("✓ Profile cached for fast access")

        # Step 7: Store contract in MongoDB
        print("\n[7/8] Storing contract documents in MongoDB...")
        contract_id = self._store_contract(subscriber_id, account_id, subscriber_data, plan_data)
        print(f"✓ Contract stored: {contract_id}")

        # Step 8: Create Neo4j relationships
        print("\n[8/8] Creating relationship graph in Neo4j...")
        self._create_neo4j_relationships(subscriber_id, subscriber_data)
        print("✓ Relationship graph created")

        # Activate subscriber
        print("\n[FINAL] Activating subscriber...")
        self._activate_subscriber(subscriber_id)
        print("✓ Subscriber activated!")

        print("\n" + "=" * 80)
        print("ONBOARDING COMPLETE!")
        print("=" * 80)
        print(f"\nSubscriber ID: {subscriber_id}")
        print(f"MSISDN: {subscriber_data['msisdn']}")
        print(f"Account: {account_id}")
        print(f"Plan: {plan_data['plan_name']}")
        print(f"Status: ACTIVE")

        return {
            'subscriber_id': subscriber_id,
            'account_id': account_id,
            'msisdn': subscriber_data['msisdn'],
            'plan': plan_data['plan_name'],
            'status': 'ACTIVE'
        }

    def _create_subscriber(self, data):
        """Create subscriber in PostgreSQL"""
        subscriber_id = str(uuid.uuid4())
        subscriber_number = f"SUB-{datetime.now().strftime('%Y%m%d')}-{subscriber_id[:8]}"

        # Generate IMSI (simplified)
        imsi = f"310150{str(uuid.uuid4().int)[:9]}"

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO subscribers (
                    subscriber_id, subscriber_number, msisdn, imsi,
                    first_name, last_name, date_of_birth, email,
                    account_type, credit_class, status, kyc_status
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                subscriber_id, subscriber_number, data['msisdn'], imsi,
                data['first_name'], data['last_name'], data['date_of_birth'],
                data['email'], data.get('account_type', 'INDIVIDUAL'),
                data.get('credit_class', 'POSTPAID'), 'PENDING', 'PENDING'
            ))
            self.pg_conn.commit()

        return subscriber_id

    def _create_account(self, subscriber_id, data):
        """Create account in PostgreSQL"""
        account_id = str(uuid.uuid4())
        account_number = f"ACC-{datetime.now().strftime('%Y%m%d')}-{account_id[:8]}"

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO accounts (
                    account_id, account_number, primary_subscriber_id,
                    account_type, billing_cycle_day, status
                ) VALUES (%s, %s, %s, %s, %s, %s)
            """, (
                account_id, account_number, subscriber_id,
                data.get('account_type', 'INDIVIDUAL'),
                data.get('billing_cycle_day', 1), 'ACTIVE'
            ))
            self.pg_conn.commit()

        return account_id

    def _add_address(self, subscriber_id, account_id, address_data):
        """Add service address"""
        address_id = str(uuid.uuid4())

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO addresses (
                    address_id, subscriber_id, account_id, address_type,
                    street_address_1, city, state, postal_code,
                    latitude, longitude, is_primary
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                address_id, subscriber_id, account_id, 'SERVICE',
                address_data['street'], address_data['city'],
                address_data['state'], address_data['zip'],
                address_data.get('latitude'), address_data.get('longitude'),
                True
            ))
            self.pg_conn.commit()

        return address_id

    def _assign_plan(self, subscriber_id, account_id, plan_data):
        """Assign service plan (simplified - assumes plan exists)"""
        # In production, would lookup plan_id from plans table
        plan_id = plan_data.get('plan_id', str(uuid.uuid4()))

        # Create initial charge for activation
        charge_id = str(uuid.uuid4())

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO charges (
                    charge_id, account_id, subscriber_id,
                    charge_type, description, amount, total_amount,
                    status, charge_date
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                charge_id, account_id, subscriber_id,
                'SUBSCRIPTION', f"Monthly charge - {plan_data['plan_name']}",
                plan_data['monthly_charge'], plan_data['monthly_charge'],
                'PENDING', date.today()
            ))

            # Add activation fee if applicable
            if plan_data.get('activation_fee', 0) > 0:
                activation_charge_id = str(uuid.uuid4())
                cursor.execute("""
                    INSERT INTO charges (
                        charge_id, account_id, subscriber_id,
                        charge_type, description, amount, total_amount,
                        status, charge_date
                    ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
                """, (
                    activation_charge_id, account_id, subscriber_id,
                    'ONE_TIME', 'Activation fee',
                    plan_data['activation_fee'], plan_data['activation_fee'],
                    'PENDING', date.today()
                ))

            self.pg_conn.commit()

        return plan_id

    def _add_payment_method(self, subscriber_id, account_id, payment_data):
        """Add payment method"""
        payment_method_id = str(uuid.uuid4())

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO payment_methods (
                    payment_method_id, account_id, subscriber_id,
                    method_type, card_last_four, card_brand,
                    card_expiry_month, card_expiry_year,
                    is_default, is_auto_pay, status
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                payment_method_id, account_id, subscriber_id,
                payment_data['method_type'], payment_data.get('card_last_four'),
                payment_data.get('card_brand'),
                payment_data.get('expiry_month'), payment_data.get('expiry_year'),
                True, payment_data.get('auto_pay', True), 'ACTIVE'
            ))
            self.pg_conn.commit()

        return payment_method_id

    def _cache_subscriber_profile(self, subscriber_id, subscriber_data, plan_data):
        """Cache subscriber profile in Redis for fast access"""

        # Cache basic profile (TTL: 1 hour)
        profile = {
            'subscriber_id': subscriber_id,
            'msisdn': subscriber_data['msisdn'],
            'first_name': subscriber_data['first_name'],
            'last_name': subscriber_data['last_name'],
            'email': subscriber_data['email'],
            'status': 'ACTIVE',
            'credit_class': subscriber_data.get('credit_class', 'POSTPAID'),
            'plan_name': plan_data['plan_name'],
            'cached_at': datetime.now().isoformat()
        }

        self.redis_client.setex(
            f"profile:subscriber:{subscriber_id}",
            3600,  # 1 hour
            json.dumps(profile)
        )

        # Cache plan limits
        self.redis_client.hset(f"plan:limits:{subscriber_id}", "voice_minutes", plan_data.get('voice_minutes', 'unlimited'))
        self.redis_client.hset(f"plan:limits:{subscriber_id}", "data_gb", plan_data.get('data_gb', 'unlimited'))
        self.redis_client.hset(f"plan:limits:{subscriber_id}", "sms_count", plan_data.get('sms_count', 'unlimited'))
        self.redis_client.expire(f"plan:limits:{subscriber_id}", 3600)

        # Initialize usage counters for current month
        month_key = datetime.now().strftime('%Y-%m')
        self.redis_client.set(f"usage:voice:{subscriber_id}:{month_key}", 0, ex=2678400)
        self.redis_client.set(f"usage:data:{subscriber_id}:{month_key}", 0, ex=2678400)
        self.redis_client.set(f"usage:sms:{subscriber_id}:{month_key}", 0, ex=2678400)

    def _store_contract(self, subscriber_id, account_id, subscriber_data, plan_data):
        """Store contract document in MongoDB"""

        contract = {
            'subscriber_id': subscriber_id,
            'account_id': account_id,
            'contract_type': 'SERVICE_AGREEMENT',
            'msisdn': subscriber_data['msisdn'],
            'customer_name': f"{subscriber_data['first_name']} {subscriber_data['last_name']}",
            'plan': {
                'name': plan_data['plan_name'],
                'monthly_charge': float(plan_data['monthly_charge']),
                'voice_minutes': plan_data.get('voice_minutes', 'unlimited'),
                'data_gb': plan_data.get('data_gb', 'unlimited'),
                'sms_count': plan_data.get('sms_count', 'unlimited')
            },
            'terms': {
                'contract_length_months': plan_data.get('contract_length', 24),
                'early_termination_fee': float(plan_data.get('etf', 200)),
                'auto_renew': True
            },
            'effective_date': datetime.now(),
            'status': 'ACTIVE',
            'signed_at': datetime.now(),
            'signature_method': 'ELECTRONIC',
            'created_at': datetime.now()
        }

        result = self.mongo_db.contracts.insert_one(contract)
        return str(result.inserted_id)

    def _create_neo4j_relationships(self, subscriber_id, subscriber_data):
        """Create subscriber node and relationships in Neo4j"""

        with self.neo4j_driver.session() as session:
            # Create subscriber node
            session.run("""
                MERGE (s:Subscriber {subscriber_id: $subscriber_id})
                SET s.msisdn = $msisdn,
                    s.name = $name,
                    s.email = $email,
                    s.status = 'ACTIVE',
                    s.created_at = datetime()
            """,
                subscriber_id=subscriber_id,
                msisdn=subscriber_data['msisdn'],
                name=f"{subscriber_data['first_name']} {subscriber_data['last_name']}",
                email=subscriber_data['email']
            )

            # If referrer exists, create referral relationship
            if subscriber_data.get('referred_by'):
                session.run("""
                    MATCH (s:Subscriber {subscriber_id: $subscriber_id})
                    MATCH (r:Subscriber {subscriber_id: $referrer_id})
                    MERGE (r)-[:REFERRED]->(s)
                """,
                    subscriber_id=subscriber_id,
                    referrer_id=subscriber_data['referred_by']
                )

    def _activate_subscriber(self, subscriber_id):
        """Activate subscriber"""
        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                UPDATE subscribers
                SET status = 'ACTIVE',
                    activation_date = CURRENT_DATE,
                    kyc_status = 'VERIFIED'
                WHERE subscriber_id = %s
            """, (subscriber_id,))
            self.pg_conn.commit()

    def close_connections(self):
        """Close all database connections"""
        self.pg_conn.close()
        self.redis_client.close()
        self.mongo_client.close()
        self.neo4j_driver.close()


def main():
    """Example usage"""

    # Sample subscriber data
    subscriber_data = {
        'msisdn': '+14155551234',
        'first_name': 'John',
        'last_name': 'Doe',
        'date_of_birth': '1990-05-15',
        'email': 'john.doe@email.com',
        'account_type': 'INDIVIDUAL',
        'credit_class': 'POSTPAID',
        'billing_cycle_day': 1,
        'address': {
            'street': '123 Market St',
            'city': 'San Francisco',
            'state': 'CA',
            'zip': '94102',
            'latitude': 37.7749,
            'longitude': -122.4194
        }
    }

    # Sample plan data
    plan_data = {
        'plan_name': 'Unlimited Premium 5G',
        'monthly_charge': Decimal('85.00'),
        'activation_fee': Decimal('35.00'),
        'voice_minutes': 'unlimited',
        'data_gb': 'unlimited',
        'sms_count': 'unlimited',
        'contract_length': 24,
        'etf': Decimal('200.00')
    }

    # Sample payment method
    payment_method = {
        'method_type': 'CREDIT_CARD',
        'card_last_four': '1234',
        'card_brand': 'VISA',
        'expiry_month': 12,
        'expiry_year': 2027,
        'auto_pay': True
    }

    # Create workflow instance
    workflow = TelcoSubscriberOnboarding()

    try:
        # Execute onboarding workflow
        result = workflow.onboard_subscriber(
            subscriber_data, plan_data, payment_method
        )

        print("\n" + "=" * 80)
        print("WORKFLOW RESULT:")
        print(json.dumps(result, indent=2, default=str))

    finally:
        workflow.close_connections()


if __name__ == '__main__':
    main()
