/**
 * Authentication Controller
 */
window.safeLauncher.controller('authController', [ '$scope', '$state', '$rootScope', '$timeout',
  'authFactory', 'CONSTANTS', 'MESSAGES', '$translate',
  function($scope, $state, $rootScope, $timeout, auth, CONSTANTS, MESSAGES, $translate) {
    var REQUEST_TIMEOUT = 90 * 1000;
    var FIELD_FOCUS_DELAY = 100;
    var showErrorField = function(targetId, msg) {
      var errorTarget = $('#' + targetId);
      errorTarget.addClass('error');
      errorTarget.children('.msg').text(msg);
      errorTarget.children('input').focus();
    };

    var removeErrorMsg = function(targetId) {
      var errorTarget = $('#' + targetId);
      errorTarget.removeClass('error');
      errorTarget.children('.msg').text('');
      errorTarget.children('input').focus();
    };

    // user create account
    var createAccount = function() {
      if ($rootScope.$networkStatus.status !== window.NETWORK_STATE.CONNECTED) {
        $translate('Network not yet connected').then(function(msg) {
          $rootScope.$toaster.show({
            msg: msg,
            hasOption: false,
            isError: true
          }, function() {});
        });
        return;
      }
      var request = new Request(onAuthResponse);
      $scope.cancelRequest = request.cancel;
      $rootScope.isAuthLoading = true;
      request.execute(function(done) {
        auth.register($scope.user.accountSecret, $scope.user.accountPassword, done);
      });
    };

    $scope.checkStateErrors = function() {
      if ($state.params.errorMsg) {
        showErrorField('AccountSecret', $state.params.errorMsg);
      }
    };

    $scope.secretValid = false;
    $scope.passwordValid = false;
    $scope.createAccFlow = {
      states: [
        'WELCOME',
        'ACC_INFO',
        'ACC_SECRET_FORM',
        'ACC_PASS',
        'ACC_PASS_FORM'
      ],
      totalCount: function() {
        return this.states.length;
      },
      currentPos: 0,
      getPos: function(state) {
        return this.states.indexOf(state);
      },
      setPos: function(state) {
        removeErrorMsg('AccountSecret');
        if (this.states.indexOf(state) > this.states.indexOf('ACC_SECRET_FORM')) {
          if (!$scope.secretValid) {
            $translate(MESSAGES.ACC_SECRET_MUST_STRONGER).then(function(msg) {
              showErrorField('AccountSecret', msg);
            });
            return;
          }
          if ($scope.user.accountSecret !== $scope.user.confirmAccountSecret) {
            $translate(MESSAGES.ENTRIES_DONT_MATCH).then(function(msg) {
              showErrorField('AccountSecretConfirm', msg);
            });
            return;
          }
        }
        $state.go('app.account', { currentPage: $state.params.currentPage,
          currentState: state, errorMsg: $state.params.errorMsg }, { notify: false });
        this.currentPos = state ? this.states.indexOf(state) : 0;
      },
      continue: function() {
        if (this.currentPos < (this.totalCount() - 1)) {
          return this.setPos(this.states[this.currentPos + 1]);
        }
        if (this.currentPos === this.states.indexOf('ACC_PASS_FORM')) {
          if (!$scope.passwordValid) {
            $translate(MESSAGES.ACC_PASS_MUST_STRONGER).then(function(msg) {
              showErrorField('AccountPass', msg);
            });
            return;
          }
          if ($scope.user.accountPassword !== $scope.user.confirmAccountPassword) {
            $translate(MESSAGES.ENTRIES_DONT_MATCH).then(function(msg) {
              showErrorField('AccountPassConfirm', msg);
            });
            return;
          }
        }
        createAccount();
      },
      back: function() {
        if (this.currentPos > 0) {
          this.currentPos--;
        }
        $state.go('app.account', { currentPage: $state.params.currentPage,
          currentState: this.states[this.currentPos], errorMsg: null }, { notify: false });
      }
    };
    var Request = function(callback) {
      var self = this;
      var alive = true;

      var onResponse = function(err) {
        if (!alive) {
          return;
        }
        alive = false;
        callback(err);
      };

      self.cancel = function() {
        $translate('Request cancelled').then(function(msg) {
          onResponse(new Error(msg));
        });
        alive = false;
      };

      self.execute = function(func) {
        func(onResponse);
      };
    };

    var onAuthResponse = function(err) {
      $rootScope.isAuthLoading = false;
      $scope.$applyAsync();
      $rootScope.userInfo = $scope.user;
      $scope.user = {};
      if (err) {
        var errMsg = window.msl.errorCodeLookup(err.errorCode || 0);
        if ($state.params.currentPage === 'register') {
          switch (errMsg) {
            case 'CoreError::RequestTimeout':
              errMsg = 'Request timed out';
              break;
            case 'CoreError::MutationFailure::MutationError::AccountExists':
              errMsg = 'This account is already taken.';
              break;
            default:
              errMsg = errMsg.replace('CoreError::', '');
          }
          $state.go('app.account', {
            currentPage: 'register',
            currentState: $scope.createAccFlow.states[2],
            errorMsg: errMsg
          }, { reload: true });
          $rootScope.user = {};
          $translate(errMsg).then(function(msg) {
              $rootScope.$toaster.show({
                msg: msg,
                isError: true
              }, function() {});
          });
          return;
        }
        switch (errMsg) {
          case 'CoreError::RequestTimeout':
            errMsg = 'Request timed out';
            break;
          case 'CoreError::GetFailure::GetError::NoSuchAccount':
          case 'CoreError::GetFailure::GetError::NoSuchData':
            errMsg = 'Account not found';
            break;
          case 'CoreError::SymmetricDecipherFailure':
            errMsg = 'Invalid password';
            break;
          default:
            errMsg = errMsg.replace('CoreError::', '');
        }
        $translate('Login failed.').then(function(msg1) {
            $translate(errMsg).then(function(msg2) {
                var msg = msg1 + " " + msg2;
                var errorTarget = $('#errorTarget');
                errorTarget.addClass('error');
                errorTarget.children('.msg').text(msg);
                errorTarget.children('input').focus();
                errorTarget.children('input').bind('keyup', function(e) {
                  errorTarget.children('.msg').text('');
                  errorTarget.removeClass('error');
                });
                $rootScope.$toaster.show({
                  msg: msg,
                  isError: true
                }, function() {});
            });
        });
        return;
      }
      $rootScope.isAuthenticated = true;
      $rootScope.$applyAsync();
      console.log('Authorised successfully!');
    };

    // user login
    $scope.login = function() {
      if ($rootScope.$networkStatus.status !== window.NETWORK_STATE.CONNECTED) {
        $translate('Network not yet conneted').then(function(msg) {
          $rootScope.$toaster.show({
            msg: msg,
            hasOption: false,
            isError: true
          }, function() {});
        });
        return;
      }
      var request = new Request(onAuthResponse);
      $scope.cancelRequest = request.cancel;
      $rootScope.isAuthLoading = true;
      request.execute(function(done) {
        auth.login($scope.user.accountSecret, $scope.user.accountPassword, done);
      });
    };

    $scope.checkPasswordValid = function(result) {
      $scope.passwordValid = result;
      $scope.$applyAsync();
    };

    $scope.checkSecretValid = function(result) {
      $scope.secretValid = result;
      $scope.$applyAsync();
    };

    $scope.createAccNavigation = function(e, state) {
      if (e.target.className.split(',').indexOf('disabled') !== -1) {
        return;
      }
      $scope.createAccCurrentState = state;
    };
  }
]);
