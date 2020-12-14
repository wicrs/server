Feature: Get a link the correct OAuth page.
Scenario: GitHub OAuth start v1.
Given I have an instance of wirc on localhost
When I perform an authenticated GET on http://localhost:24816/api/v1/account
Then I should see thing
