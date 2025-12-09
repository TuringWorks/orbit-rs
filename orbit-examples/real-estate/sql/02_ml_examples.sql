WITH features AS (
  SELECT l.listing_id,
         l.bedrooms,
         l.bathrooms,
         l.area_sqft,
         l.year_built,
         l.price AS label
  FROM listings l
)
SELECT ML_TRAIN_MODEL('price_lr','gradient_boosting',
       ARRAY[bedrooms, bathrooms, area_sqft, year_built], label)
FROM features;
WITH predict AS (
  SELECT 100 AS listing_id, 3 AS bedrooms, 2.0 AS bathrooms, 1800 AS area_sqft, 2000 AS year_built
)
SELECT listing_id,
       ML_PREDICT('price_lr', ARRAY[bedrooms, bathrooms, area_sqft, year_built]) AS predicted_price
FROM predict;
