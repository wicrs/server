Feature: Get account info.
    Scenario: Get authenticated account v1.
        Given I have an instance of wirc on localhost
        When I perform an authenticated GET on http://localhost:24816/api/v1/account
        Then I should see {"id":"testaccount","email":"test@example.com","created":0,"service":"testing","users":{}}
        
    Scenario: Get unauthenticated account v1.
        Given I have an instance of wirc on localhost
        When I perform a GET on http://localhost:24816/api/v1/account/testaccount
        Then I should see {"id":"testaccount","created":0,"users":{}}
