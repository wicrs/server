Feature: Get a link the correct OAuth page.
Scenario: GitHub OAuth start v1.
Given I have an instance of wirc on localhost
When I GET api/v1/login/github
Then I should see a redirect to https://github.com/login
