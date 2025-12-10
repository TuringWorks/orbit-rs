-- AdTech: Campaign & Inventory Management (SQL)
-- Purpose: Manage the core "Buy Side" (Advertisers, Campaigns) and "Sell Side" (Publishers, Ad Units) data.
-- This schema handles the setup, targeting, and billing aspects of the Ad Network.
-- ==========================================
-- BUY SIDE (Advertisers / DSP)
-- ==========================================
CREATE TABLE advertisers (
    advertiser_id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    industry VARCHAR(100),
    -- e.g., 'Retail', 'Auto'
    total_spend DECIMAL(15, 2) DEFAULT 0.00,
    status VARCHAR(50) DEFAULT 'ACTIVE'
);
CREATE TABLE campaigns (
    campaign_id UUID PRIMARY KEY,
    advertiser_id UUID REFERENCES advertisers(advertiser_id),
    name VARCHAR(255) NOT NULL,
    start_date TIMESTAMP,
    end_date TIMESTAMP,
    total_budget DECIMAL(15, 2),
    daily_budget DECIMAL(15, 2),
    status VARCHAR(50) -- 'RUNNING', 'PAUSED', 'COMPLETED'
);
CREATE TABLE line_items (
    line_item_id UUID PRIMARY KEY,
    campaign_id UUID REFERENCES campaigns(campaign_id),
    name VARCHAR(255),
    bid_price DECIMAL(10, 4),
    -- e.g., CPM price
    bid_type VARCHAR(50),
    -- 'CPM', 'CPC', 'CPA'
    targeting_geo JSONB,
    -- e.g. ["US", "GB"]
    targeting_device JSONB,
    -- e.g. ["Mobile", "Desktop"]
    creative_ids UUID [] -- Array of creatives linked to this line item
);
CREATE TABLE creatives (
    creative_id UUID PRIMARY KEY,
    advertiser_id UUID REFERENCES advertisers(advertiser_id),
    name VARCHAR(255),
    type VARCHAR(50),
    -- 'BANNER', 'VIDEO', 'NATIVE'
    url TEXT,
    -- Destination URL
    asset_url TEXT,
    -- Image/Video source
    width INT,
    height INT,
    status VARCHAR(50)
);
-- ==========================================
-- SELL SIDE (Publishers / SSP)
-- ==========================================
CREATE TABLE publishers (
    publisher_id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    payout_rate DECIMAL(5, 4),
    -- e.g., 0.70 (70% rev share)
    integration_type VARCHAR(50) -- 'Prebid', 'Direct', 'S2S'
);
CREATE TABLE sites (
    site_id UUID PRIMARY KEY,
    publisher_id UUID REFERENCES publishers(publisher_id),
    domain VARCHAR(255) NOT NULL,
    category VARCHAR(100),
    -- e.g., 'News', 'Sports'
    monthly_traffic BIGINT
);
CREATE TABLE ad_units (
    ad_unit_id UUID PRIMARY KEY,
    site_id UUID REFERENCES sites(site_id),
    name VARCHAR(255),
    size_width INT,
    size_height INT,
    floor_price DECIMAL(10, 4),
    -- Minimum bid accepted
    format VARCHAR(50) -- 'Display', 'Video'
);
-- ==========================================
-- ANALYTICS (Aggregated Stats)
-- ==========================================
CREATE TABLE daily_performance_stats (
    date DATE,
    campaign_id UUID,
    ad_unit_id UUID,
    impressions BIGINT DEFAULT 0,
    clicks BIGINT DEFAULT 0,
    revenue DECIMAL(15, 4) DEFAULT 0.00,
    spend DECIMAL(15, 4) DEFAULT 0.00,
    PRIMARY KEY (date, campaign_id, ad_unit_id)
);
-- Example Data
INSERT INTO advertisers (advertiser_id, name, industry)
VALUES (
        '11111111-1111-1111-1111-111111111111',
        'Acme Motors',
        'Automotive'
    );
INSERT INTO campaigns (
        campaign_id,
        advertiser_id,
        name,
        total_budget,
        status
    )
VALUES (
        '22222222-2222-2222-2222-222222222222',
        '11111111-1111-1111-1111-111111111111',
        'Summer SUV Sale',
        50000.00,
        'RUNNING'
    );
INSERT INTO publishers (publisher_id, name)
VALUES (
        '33333333-3333-3333-3333-333333333333',
        'Global News Corp'
    );