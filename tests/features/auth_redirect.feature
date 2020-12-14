Feature: Get a link the correct OAuth page.
Scenario: GitHub OAuth start v1.
Given I have an instance of wirc on localhost
When I GET http://localhost:24816/api/v1/login/github
Then I should be redirected to https://github.com/login
